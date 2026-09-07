// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Planning, performing, verifying, and recording a publication.
//!
//! This is the part that decides what happens; [`super::api`] only carries it
//! out. Everything here works against the [`Destination`] trait, so the rules
//! that matter, that nothing is recorded until it is confirmed and that the
//! video ID is written down the moment it exists, are tested without a
//! network.

use super::{Destination, UploadRequest, VideoState};
use crate::channel::ChannelConfig;
use crate::document::Document;
use crate::episode;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// A step that changes the destination.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub enum Step {
    Upload,
    Captions,
    Thumbnail,
    Playlist(String),
}

impl Step {
    fn describe(&self) -> String {
        match self {
            Step::Upload => "upload the video".to_owned(),
            Step::Captions => "upload captions".to_owned(),
            Step::Thumbnail => "set the thumbnail".to_owned(),
            Step::Playlist(id) => format!("add to playlist {id}"),
        }
    }
}

/// What an upload would do, assembled without touching the network.
#[derive(Debug, Serialize)]
pub struct Plan {
    pub episode: PathBuf,
    pub base: String,
    pub request: UploadRequest,
    pub steps: Vec<Step>,
    pub captions: Option<PathBuf>,
    pub thumbnail: Option<PathBuf>,
    pub playlists: Vec<String>,
    /// Reasons the episode is not ready, which make the plan unusable.
    pub blockers: Vec<String>,
}

impl Plan {
    pub fn as_text(&self) -> String {
        let mut lines = vec![
            format!("Upload plan for {}", self.base),
            format!("  Title:    {}", self.request.title),
            format!("  File:     {}", self.request.file.display()),
            format!("  Privacy:  {}", self.request.privacy_status),
            format!("  Tags:     {} tags", self.request.tags.len()),
            format!(
                "  Captions: {}",
                self.captions
                    .as_ref()
                    .map_or("none".to_owned(), |path| path.display().to_string())
            ),
            format!(
                "  Thumbnail:{}",
                self.thumbnail
                    .as_ref()
                    .map_or(" none".to_owned(), |path| format!(" {}", path.display()))
            ),
        ];
        lines.push("  Steps:".to_owned());
        lines.extend(
            self.steps
                .iter()
                .map(|step| format!("    {}", step.describe())),
        );
        if !self.blockers.is_empty() {
            lines.push("  Blocked:".to_owned());
            lines.extend(self.blockers.iter().map(|reason| format!("    {reason}")));
        }
        lines.join("\n")
    }
}

/// Build the plan from local state only.
pub fn plan(
    episode_path: &Path,
    config: &ChannelConfig,
    playlists: &[String],
    privacy: Option<&str>,
) -> Result<Plan> {
    let report = episode::inspect(episode_path)?;
    let base = report.base.clone();
    let directory = report.episode_path.clone();

    let mut blockers = Vec::new();
    let file = ["processed.mp4", "mp4"]
        .iter()
        .map(|extension| directory.join(format!("{base}.{extension}")))
        .find(|path| path.is_file());
    let Some(file) = file else {
        bail!("no video to upload: expected {base}.processed.mp4 or {base}.mp4");
    };

    let title = report
        .fields
        .title
        .clone()
        .context("the episode record has no title")?;
    let description =
        std::fs::read_to_string(directory.join(format!("{base}.youtube-description.txt")))
            .unwrap_or_else(|_| {
                blockers.push("no formatted description".to_owned());
                String::new()
            });
    let tags = std::fs::read_to_string(directory.join(format!("{base}.tags.txt")))
        .map(|text| {
            text.trim()
                .split(',')
                .map(|tag| tag.trim().to_owned())
                .filter(|tag| !tag.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|_| {
            blockers.push("no tag list".to_owned());
            Vec::new()
        });

    if let Some(video_id) = &report.fields.video_id {
        blockers.push(format!(
            "already recorded as uploaded ({video_id}); uploading again would create a duplicate"
        ));
    }

    let youtube = config.youtube.as_ref();
    let privacy = privacy
        .map(str::to_owned)
        .or_else(|| youtube.and_then(|y| y.default_privacy.clone()))
        // An upload defaults to unlisted so a mistake is never immediately
        // public. Making a video public is a separate, deliberate action.
        .unwrap_or_else(|| "unlisted".to_owned());
    super::api::validate_privacy(&privacy)?;

    let captions = Some(directory.join(format!("{base}.srt"))).filter(|path| path.is_file());
    let thumbnail =
        Some(directory.join(format!("{base}.thumbnail.png"))).filter(|path| path.is_file());

    let mut steps = vec![Step::Upload];
    if captions.is_some() {
        steps.push(Step::Captions);
    }
    if thumbnail.is_some() {
        steps.push(Step::Thumbnail);
    }
    steps.extend(playlists.iter().map(|id| Step::Playlist(id.clone())));
    let session_file = directory.join(format!(".{base}.youtube-upload-session"));

    Ok(Plan {
        episode: directory,
        base,
        request: UploadRequest {
            file,
            session_file,
            title,
            description,
            tags,
            category_id: youtube
                .and_then(|y| y.category_id)
                .unwrap_or(28)
                .to_string(),
            privacy_status: privacy,
            language: youtube.and_then(|y| y.language.clone()),
            license: youtube.and_then(|y| y.license_id.clone()),
        },
        steps,
        captions,
        thumbnail,
        playlists: playlists.to_vec(),
        blockers,
    })
}

/// What actually happened, step by step.
#[derive(Debug, Default, Serialize)]
pub struct Outcome {
    pub video_id: Option<String>,
    pub confirmed: Vec<String>,
    pub failed: Vec<String>,
    pub recorded: Vec<String>,
}

/// One independently retryable operation for an already uploaded episode.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub enum RepairKind {
    Captions,
    Thumbnail,
    Playlist(String),
}

#[derive(Debug, Serialize)]
pub struct RepairPlan {
    pub episode: PathBuf,
    pub base: String,
    pub video_id: String,
    pub kind: RepairKind,
    pub asset: Option<PathBuf>,
    pub language: String,
}

impl RepairPlan {
    pub fn as_text(&self) -> String {
        let action = match &self.kind {
            RepairKind::Captions => format!("upload captions from {}", self.asset_display()),
            RepairKind::Thumbnail => format!("set thumbnail from {}", self.asset_display()),
            RepairKind::Playlist(id) => format!("add to playlist {id}"),
        };
        format!(
            "Repair plan for {}\n  Video: {}\n  Action: {action}",
            self.base, self.video_id
        )
    }

    fn asset_display(&self) -> String {
        self.asset
            .as_ref()
            .map_or_else(|| "none".to_owned(), |path| path.display().to_string())
    }
}

pub fn plan_repair(
    episode_path: &Path,
    config: &ChannelConfig,
    kind: RepairKind,
) -> Result<RepairPlan> {
    let report = episode::inspect(episode_path)?;
    let video_id = report
        .fields
        .video_id
        .context("this episode has no recorded video ID")?;
    let asset = match kind {
        RepairKind::Captions => Some(report.episode_path.join(format!("{}.srt", report.base))),
        RepairKind::Thumbnail => Some(
            report
                .episode_path
                .join(format!("{}.thumbnail.png", report.base)),
        ),
        RepairKind::Playlist(_) => None,
    };
    if let Some(path) = &asset
        && !path.is_file()
    {
        bail!("repair asset does not exist: {}", path.display());
    }
    let language = config
        .youtube
        .as_ref()
        .and_then(|youtube| youtube.language.clone())
        .unwrap_or_else(|| "en".to_owned());
    Ok(RepairPlan {
        episode: report.episode_path,
        base: report.base,
        video_id,
        kind,
        asset,
        language,
    })
}

pub fn apply_repair(
    destination: &dyn Destination,
    plan: &RepairPlan,
    timestamp: &str,
) -> Result<Outcome> {
    let mut outcome = Outcome {
        video_id: Some(plan.video_id.clone()),
        ..Outcome::default()
    };
    match &plan.kind {
        RepairKind::Captions => {
            let track = plan
                .asset
                .as_deref()
                .context("caption repair has no track")?;
            destination.upload_caption(&plan.video_id, track, &plan.language, "English")?;
            let count = destination.caption_count(&plan.video_id)?;
            if count == 0 {
                outcome
                    .failed
                    .push("captions uploaded but YouTube reports none".to_owned());
            } else {
                outcome
                    .confirmed
                    .push(format!("captions ({count} track(s))"));
                record_repair_field(
                    plan,
                    "Captions",
                    &format!("uploaded {timestamp}"),
                    &mut outcome,
                );
            }
        }
        RepairKind::Thumbnail => {
            let image = plan
                .asset
                .as_deref()
                .context("thumbnail repair has no image")?;
            destination.set_thumbnail(&plan.video_id, image)?;
            outcome.confirmed.push("thumbnail (unverified)".to_owned());
            record_repair_field(
                plan,
                "Uploaded",
                &format!("{timestamp} (unverified)"),
                &mut outcome,
            );
        }
        RepairKind::Playlist(playlist) => {
            destination.add_to_playlist(playlist, &plan.video_id)?;
            if destination.playlist_contains(playlist, &plan.video_id)? {
                outcome.confirmed.push(format!("playlist {playlist}"));
                record_repair_playlist(plan, playlist, timestamp, &mut outcome);
            } else {
                outcome.failed.push(format!(
                    "playlist {playlist} was added but YouTube does not report the video in it"
                ));
            }
        }
    }
    Ok(outcome)
}

fn repair_metadata_path(plan: &RepairPlan) -> PathBuf {
    plan.episode.join(format!("{}.episode-data.md", plan.base))
}

fn record_repair_field(plan: &RepairPlan, label: &str, value: &str, outcome: &mut Outcome) {
    let Ok(mut document) = Document::read(&repair_metadata_path(plan)) else {
        return;
    };
    let section = (label == "Uploaded").then_some("Thumbnail");
    if let Ok(Some(_)) = document.set_field(label, section, value, true)
        && document.write().is_ok()
    {
        outcome.recorded.push(label.to_owned());
    }
}

fn record_repair_playlist(
    plan: &RepairPlan,
    playlist: &str,
    timestamp: &str,
    outcome: &mut Outcome,
) {
    let Ok(mut document) = Document::read(&repair_metadata_path(plan)) else {
        return;
    };
    if let Ok(Some(_)) = document.mark_playlist(playlist, Some(&format!("Added {timestamp}")))
        && document.write().is_ok()
    {
        outcome.recorded.push(format!("playlist {playlist}"));
    }
}

impl Outcome {
    pub fn as_text(&self) -> String {
        let mut lines = vec![match &self.video_id {
            Some(id) => format!("Video ID: {id}"),
            None => "No video was created".to_owned(),
        }];
        for item in &self.confirmed {
            lines.push(format!("  confirmed: {item}"));
        }
        for item in &self.failed {
            lines.push(format!("  FAILED: {item}"));
        }
        for item in &self.recorded {
            lines.push(format!("  recorded: {item}"));
        }
        lines.join("\n")
    }

    pub fn is_complete(&self) -> bool {
        self.failed.is_empty() && self.video_id.is_some()
    }
}

/// Carry out a plan, recording only what the destination confirms.
///
/// The video ID is written to the episode record as soon as the upload
/// returns, before anything else is attempted: everything after that point can
/// be retried, but an ID that was never written down cannot be recovered from
/// the workspace.
pub fn apply(destination: &dyn Destination, plan: &Plan, timestamp: &str) -> Result<Outcome> {
    if !plan.blockers.is_empty() {
        bail!(
            "this episode is not ready to upload: {}",
            plan.blockers.join("; ")
        );
    }
    let mut outcome = Outcome::default();

    let video_id = destination.upload_video(&plan.request)?;
    outcome.video_id = Some(video_id.clone());
    outcome.confirmed.push("upload".to_owned());
    record_identity(plan, &video_id, timestamp, &mut outcome)?;

    if let Some(track) = &plan.captions {
        match destination.upload_caption(&video_id, track, "en", "English") {
            Ok(()) => match destination.caption_count(&video_id) {
                Ok(count) if count > 0 => {
                    outcome
                        .confirmed
                        .push(format!("captions ({count} track(s))"));
                    record_field(
                        plan,
                        "Captions",
                        &format!("uploaded {timestamp}"),
                        &mut outcome,
                    );
                }
                Ok(_) => outcome
                    .failed
                    .push("captions uploaded but YouTube reports none".to_owned()),
                Err(error) => outcome.failed.push(format!("caption check: {error:#}")),
            },
            Err(error) => outcome.failed.push(format!("captions: {error:#}")),
        }
    }

    if let Some(image) = &plan.thumbnail {
        match destination.set_thumbnail(&video_id, image) {
            // A thumbnail cannot be read back through the API, so the API's own
            // answer is recorded and labelled rather than presented as verified.
            Ok(()) => {
                outcome.confirmed.push("thumbnail (unverified)".to_owned());
                record_field(
                    plan,
                    "Uploaded",
                    &format!("{timestamp} (unverified)"),
                    &mut outcome,
                );
            }
            Err(error) => outcome.failed.push(format!("thumbnail: {error:#}")),
        }
    }

    for playlist in &plan.playlists {
        match destination.add_to_playlist(playlist, &video_id) {
            Ok(()) => match destination.playlist_contains(playlist, &video_id) {
                Ok(true) => {
                    outcome.confirmed.push(format!("playlist {playlist}"));
                    record_playlist(plan, playlist, timestamp, &mut outcome);
                }
                Ok(false) => outcome.failed.push(format!(
                    "playlist {playlist} was added but YouTube does not report the video in it"
                )),
                Err(error) => outcome
                    .failed
                    .push(format!("playlist {playlist} check: {error:#}")),
            },
            Err(error) => outcome
                .failed
                .push(format!("playlist {playlist}: {error:#}")),
        }
    }

    // Privacy is read back rather than assumed from what was requested.
    match destination.video(&video_id) {
        Ok(Some(state)) => {
            if let Some(privacy) = state.privacy_status {
                record_field(plan, "Privacy Status", &privacy, &mut outcome);
                outcome.confirmed.push(format!("privacy {privacy}"));
            }
        }
        Ok(None) => outcome
            .failed
            .push("the uploaded video could not be read back".to_owned()),
        Err(error) => outcome.failed.push(format!("verification: {error:#}")),
    }

    Ok(outcome)
}

fn metadata_path(plan: &Plan) -> PathBuf {
    plan.episode.join(format!("{}.episode-data.md", plan.base))
}

/// Write the facts that cannot be recovered locally.
fn record_identity(
    plan: &Plan,
    video_id: &str,
    timestamp: &str,
    outcome: &mut Outcome,
) -> Result<()> {
    let path = metadata_path(plan);
    let mut document = Document::read(&path)?;
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let short = format!("https://youtu.be/{video_id}");
    let uploaded = plan
        .request
        .file
        .file_name()
        .map(|name| format!("`{}`", name.to_string_lossy()))
        .unwrap_or_default();

    let mut wrote = Vec::new();
    for (label, value) in [
        ("Video ID", video_id),
        ("YouTube URL", url.as_str()),
        ("YouTube Short URL", short.as_str()),
        ("Upload Date", timestamp),
        ("Uploaded File", uploaded.as_str()),
    ] {
        // A document that predates a field is not a failure; the write is
        // simply not possible and is left unrecorded.
        if let Ok(Some(_)) = document.set_field(label, None, value, false) {
            wrote.push(label.to_owned());
        }
    }
    document.write().with_context(|| {
        format!(
            "the video uploaded as {video_id} but {} could not be updated",
            path.display()
        )
    })?;
    outcome.recorded.extend(wrote);
    Ok(())
}

fn record_field(plan: &Plan, label: &str, value: &str, outcome: &mut Outcome) {
    let path = metadata_path(plan);
    let Ok(mut document) = Document::read(&path) else {
        return;
    };
    let section = (label == "Uploaded").then_some("Thumbnail");
    if let Ok(Some(_)) = document.set_field(label, section, value, true)
        && document.write().is_ok()
    {
        outcome.recorded.push(label.to_owned());
    }
}

fn record_playlist(plan: &Plan, playlist: &str, timestamp: &str, outcome: &mut Outcome) {
    let path = metadata_path(plan);
    let Ok(mut document) = Document::read(&path) else {
        return;
    };
    if let Ok(Some(_)) = document.mark_playlist(playlist, Some(&format!("Added {timestamp}")))
        && document.write().is_ok()
    {
        outcome.recorded.push(format!("playlist {playlist}"));
    }
}

/// A read-only reconciliation report.
#[derive(Debug, Serialize)]
pub struct Verification {
    pub base: String,
    pub video_id: String,
    pub video: Option<VideoState>,
    pub captions: usize,
    pub captions_expected: bool,
    pub playlists: Vec<PlaylistState>,
    pub issues: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PlaylistState {
    pub name: String,
    pub playlist_id: String,
    pub contains_video: bool,
}

impl Verification {
    pub fn as_text(&self) -> String {
        let mut lines = vec![format!("Verification for {}", self.base)];
        match &self.video {
            Some(video) => {
                lines.push(format!(
                    "  Video:    {} ({})",
                    video.id,
                    video.title.as_deref().unwrap_or("untitled")
                ));
                lines.push(format!(
                    "  Privacy:  {}",
                    video.privacy_status.as_deref().unwrap_or("unknown")
                ));
            }
            None => lines.push(format!("  Video:    {} NOT FOUND", self.video_id)),
        }
        lines.push(format!(
            "  Captions: {} track(s){}",
            self.captions,
            if self.captions_expected && self.captions == 0 {
                " (an SRT exists locally)"
            } else {
                ""
            }
        ));
        for playlist in &self.playlists {
            lines.push(format!(
                "  Playlist: {} {}",
                if playlist.contains_video {
                    "yes"
                } else {
                    "no "
                },
                playlist.name
            ));
        }
        lines.extend(self.issues.iter().map(|issue| format!("  ISSUE: {issue}")));
        lines.join("\n")
    }
}

/// Compare the episode record against the destination, changing nothing.
pub fn verify(
    destination: &dyn Destination,
    episode_path: &Path,
    config: &ChannelConfig,
) -> Result<Verification> {
    let report = episode::inspect(episode_path)?;
    let video_id = report
        .fields
        .video_id
        .clone()
        .context("this episode has no recorded video ID, so there is nothing to verify")?;

    let video = destination.video(&video_id)?;
    let mut issues = Vec::new();
    if video.is_none() {
        issues.push(format!(
            "no video with ID {video_id} exists on the destination"
        ));
    }
    if let Some(state) = &video
        && let (Some(remote), Some(local)) = (&state.privacy_status, &report.fields.privacy_status)
        && !local.to_ascii_lowercase().starts_with(remote.as_str())
    {
        issues.push(format!(
            "recorded privacy is '{local}' but the destination says '{remote}'"
        ));
    }

    let captions = destination.caption_count(&video_id).unwrap_or(0);
    let captions_expected = report
        .episode_path
        .join(format!("{}.srt", report.base))
        .is_file();
    if captions_expected && captions == 0 {
        issues.push("an SRT exists locally but the video has no captions".to_owned());
    }

    let mut playlists = Vec::new();
    if let Some(policy) = &config.playlists {
        let mut candidates = Vec::new();
        if let Some(id) = &policy.binge_watch_id {
            candidates.push(("Every episode".to_owned(), id.clone()));
        }
        candidates.extend(
            policy
                .topic
                .iter()
                .map(|(name, id)| (name.clone(), id.clone())),
        );
        for (name, playlist_id) in candidates {
            let contains = destination
                .playlist_contains(&playlist_id, &video_id)
                .unwrap_or(false);
            playlists.push(PlaylistState {
                name,
                playlist_id,
                contains_video: contains,
            });
        }
    }

    Ok(Verification {
        base: report.base,
        video_id,
        video,
        captions,
        captions_expected,
        playlists,
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::youtube::fake::FakeDestination;
    use std::collections::BTreeSet;

    const BASE: &str = "2026.08.07-fixture";
    const PLAYLIST: &str = "PLfixturebinge";

    struct Episode {
        _temp: tempfile::TempDir,
        path: PathBuf,
    }

    impl Episode {
        /// A complete, uploadable episode.
        fn ready() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("episodes/in-progress").join(BASE);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(
                path.join(format!("{BASE}.episode-data.md")),
                format!(
                    "# Episode Data: Fixture\n\n\
                     ## Episode Details\n\n\
                     - **Episode Date**: 2026-08-07\n\
                     - **Title**: Fixture Episode\n\n\
                     ## File Locations\n\n\
                     - **Uploaded File**: [To be added]\n\n\
                     ## YouTube\n\n### Video\n\n\
                     - **Video ID**: [To be added]\n\
                     - **YouTube URL**: [To be added]\n\
                     - **YouTube Short URL**: [To be added]\n\
                     - **Upload Date**: [To be added]\n\
                     - **Privacy Status**: [unlisted/public]\n\
                     - **Captions**: [To be added]\n\n\
                     ### Playlists to Add\n\
                     - [ ] Every episode ({PLAYLIST})\n\n\
                     ### Thumbnail\n\
                     - **Uploaded**: [To be added]\n\n\
                     ## Notes\n\nHuman prose that must survive.\n"
                ),
            )
            .unwrap();
            for name in [
                format!("{BASE}.processed.mp4"),
                format!("{BASE}.youtube-description.txt"),
                format!("{BASE}.srt"),
                format!("{BASE}.thumbnail.png"),
            ] {
                std::fs::write(path.join(name), "content").unwrap();
            }
            std::fs::write(
                path.join(format!("{BASE}.tags.txt")),
                "one,two,three,everything digital health",
            )
            .unwrap();
            Self { _temp: temp, path }
        }

        fn record(&self) -> String {
            std::fs::read_to_string(self.path.join(format!("{BASE}.episode-data.md"))).unwrap()
        }

        fn mark_uploaded(&self) {
            let path = self.path.join(format!("{BASE}.episode-data.md"));
            let text = std::fs::read_to_string(&path).unwrap().replace(
                "- **Video ID**: [To be added]",
                "- **Video ID**: existing-video",
            );
            std::fs::write(path, text).unwrap();
        }
    }

    fn config() -> ChannelConfig {
        toml::from_str(
            "[channel]\nid = 'fixture'\nname = 'Fixture'\ntimezone = 'UTC'\n\
             [youtube]\nchannel_id = 'UCfixture'\ndefault_privacy = 'unlisted'\ncategory_id = 28\n\
             [playlists]\nbinge_watch_id = 'PLfixturebinge'\n",
        )
        .unwrap()
    }

    fn failing(operations: &[&str]) -> FakeDestination {
        FakeDestination {
            failing: operations.iter().map(|name| (*name).to_owned()).collect(),
            ..FakeDestination::default()
        }
    }

    #[test]
    fn an_upload_defaults_to_unlisted_so_a_mistake_is_never_public() {
        let episode = Episode::ready();
        let plan = plan(&episode.path, &config(), &[], None).unwrap();
        assert_eq!(plan.request.privacy_status, "unlisted");
        assert!(plan.blockers.is_empty());
        assert_eq!(plan.request.tags.len(), 4);
        // The upload master is preferred over the edited one.
        assert!(plan.request.file.ends_with(format!("{BASE}.processed.mp4")));
        assert!(
            plan.request
                .session_file
                .ends_with(format!(".{BASE}.youtube-upload-session"))
        );
    }

    #[test]
    fn planning_refuses_to_upload_the_same_episode_twice() {
        let episode = Episode::ready();
        let path = episode.path.join(format!("{BASE}.episode-data.md"));
        let text = std::fs::read_to_string(&path).unwrap().replace(
            "- **Video ID**: [To be added]",
            "- **Video ID**: already123",
        );
        std::fs::write(&path, text).unwrap();

        let plan = plan(&episode.path, &config(), &[], None).unwrap();
        assert!(
            plan.blockers.iter().any(|b| b.contains("already123")),
            "{:?}",
            plan.blockers
        );
        // And applying it is refused outright.
        let error = apply(&FakeDestination::default(), &plan, "2026-08-07T09:00:00Z")
            .unwrap_err()
            .to_string();
        assert!(error.contains("not ready to upload"));
    }

    #[test]
    fn a_successful_upload_records_only_confirmed_results() {
        let episode = Episode::ready();
        let plan = plan(&episode.path, &config(), &[PLAYLIST.to_owned()], None).unwrap();
        let destination = FakeDestination::default();
        let outcome = apply(&destination, &plan, "2026-08-07T09:00:00Z").unwrap();

        assert!(outcome.is_complete(), "{:?}", outcome.failed);
        let record = episode.record();
        assert!(record.contains("- **Video ID**: fake-video-1"));
        assert!(record.contains("https://www.youtube.com/watch?v=fake-video-1"));
        assert!(record.contains(&format!("- [x] Every episode ({PLAYLIST}) - Added")));
        assert!(record.contains("- **Privacy Status**: unlisted"));
        assert!(record.contains(&format!("- **Uploaded File**: `{BASE}.processed.mp4`")));
        assert!(record.contains("Human prose that must survive."));
    }

    #[test]
    fn captions_can_be_repaired_and_are_recorded_only_after_read_back() {
        let episode = Episode::ready();
        episode.mark_uploaded();
        let plan = plan_repair(&episode.path, &config(), RepairKind::Captions).unwrap();
        let destination = FakeDestination::default();

        let outcome = apply_repair(&destination, &plan, "2026-08-07T09:00:00Z").unwrap();

        assert!(outcome.failed.is_empty());
        assert!(
            outcome
                .confirmed
                .iter()
                .any(|item| item.contains("captions"))
        );
        assert!(
            episode
                .record()
                .contains("- **Captions**: uploaded 2026-08-07")
        );
    }

    #[test]
    fn a_playlist_repair_is_verified_before_its_checkbox_is_ticked() {
        let episode = Episode::ready();
        episode.mark_uploaded();
        let plan = plan_repair(
            &episode.path,
            &config(),
            RepairKind::Playlist(PLAYLIST.to_owned()),
        )
        .unwrap();
        let destination = FakeDestination::default();

        let outcome = apply_repair(&destination, &plan, "2026-08-07T09:00:00Z").unwrap();

        assert!(outcome.failed.is_empty());
        assert!(
            episode
                .record()
                .contains(&format!("- [x] Every episode ({PLAYLIST}) - Added"))
        );
    }

    #[test]
    fn thumbnail_repair_records_that_api_acceptance_cannot_be_verified() {
        let episode = Episode::ready();
        episode.mark_uploaded();
        let plan = plan_repair(&episode.path, &config(), RepairKind::Thumbnail).unwrap();

        let outcome =
            apply_repair(&FakeDestination::default(), &plan, "2026-08-07T09:00:00Z").unwrap();

        assert!(outcome.failed.is_empty());
        assert!(
            outcome
                .confirmed
                .contains(&"thumbnail (unverified)".to_owned())
        );
        assert!(
            episode
                .record()
                .contains("2026-08-07T09:00:00Z (unverified)")
        );
    }

    #[test]
    fn the_video_id_is_recorded_even_when_every_later_step_fails() {
        // The one fact that cannot be recovered from the workspace.
        let episode = Episode::ready();
        let plan = plan(&episode.path, &config(), &[PLAYLIST.to_owned()], None).unwrap();
        let destination = failing(&[
            "upload_caption",
            "set_thumbnail",
            "add_to_playlist",
            "video",
        ]);
        let outcome = apply(&destination, &plan, "2026-08-07T09:00:00Z").unwrap();

        assert_eq!(outcome.video_id.as_deref(), Some("fake-video-1"));
        assert!(!outcome.is_complete());
        assert_eq!(outcome.failed.len(), 4);
        assert!(episode.record().contains("- **Video ID**: fake-video-1"));
    }

    #[test]
    fn a_playlist_the_destination_does_not_confirm_is_not_recorded() {
        let episode = Episode::ready();
        let plan = plan(&episode.path, &config(), &[PLAYLIST.to_owned()], None).unwrap();
        // The insert succeeds but the video is never actually in the playlist.
        let destination = FakeDestination {
            playlists: std::cell::RefCell::new(BTreeSet::new()),
            failing: ["playlist_contains".to_owned()].into_iter().collect(),
            ..FakeDestination::default()
        };
        let outcome = apply(&destination, &plan, "2026-08-07T09:00:00Z").unwrap();

        assert!(outcome.failed.iter().any(|f| f.contains("playlist")));
        assert!(
            episode
                .record()
                .contains(&format!("- [ ] Every episode ({PLAYLIST})")),
            "an unconfirmed playlist must stay unticked"
        );
    }

    #[test]
    fn captions_the_destination_does_not_report_are_not_recorded() {
        let episode = Episode::ready();
        let plan = plan(&episode.path, &config(), &[], None).unwrap();
        // Upload succeeds; the destination then reports no caption tracks.
        let destination = failing(&["caption_count"]);
        let outcome = apply(&destination, &plan, "2026-08-07T09:00:00Z").unwrap();

        assert!(outcome.failed.iter().any(|f| f.contains("caption")));
        assert!(episode.record().contains("- **Captions**: [To be added]"));
    }

    #[test]
    fn verification_reports_a_record_that_disagrees_with_the_destination() {
        let episode = Episode::ready();
        let path = episode.path.join(format!("{BASE}.episode-data.md"));
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("- **Video ID**: [To be added]", "- **Video ID**: live123")
            .replace(
                "- **Privacy Status**: [unlisted/public]",
                "- **Privacy Status**: public",
            );
        std::fs::write(&path, text).unwrap();

        let destination = FakeDestination::default();
        destination.videos.borrow_mut().push(VideoState {
            id: "live123".to_owned(),
            title: Some("Fixture Episode".to_owned()),
            privacy_status: Some("unlisted".to_owned()),
            published_at: None,
        });

        let report = verify(&destination, &episode.path, &config()).unwrap();
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.contains("recorded privacy is 'public'")),
            "{:?}",
            report.issues
        );
        // The local SRT exists but the destination has no captions.
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.contains("no captions"))
        );
        assert!(!report.playlists[0].contains_video);
    }

    #[test]
    fn verification_refuses_to_guess_when_nothing_was_uploaded() {
        let episode = Episode::ready();
        let error = verify(&FakeDestination::default(), &episode.path, &config())
            .unwrap_err()
            .to_string();
        assert!(error.contains("no recorded video ID"));
    }
}
