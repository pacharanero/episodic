// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Milestones: how far an episode has progressed within its stage.
//!
//! A milestone is derived from the files in the episode folder and the
//! findings local inspection already produced. It is computed on every read
//! and never stored, for the same reason stages are folders: there is no
//! status field to disagree with the filesystem.
//!
//! Derivation runs no tools and reads no media. A milestone is therefore never
//! reported as passing a contract it was not checked against: `present` means
//! the artefacts exist, and `failing` means inspection found a problem with
//! them.

use crate::episode::{Artefact, EpisodeFields, Finding};
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// The artefacts it checks do not exist.
    NotStarted,
    /// The artefacts exist; inspection found no problem with them.
    Present,
    /// An artefact is older than the input it was derived from.
    PossiblyStale,
    /// Inspection found a problem, or the set is incomplete.
    Failing,
    /// Not expected at this point, for example media removed by retention
    /// after publication.
    NotApplicable,
}

impl State {
    fn as_text(self) -> &'static str {
        match self {
            Self::NotStarted => "not started",
            Self::Present => "present",
            Self::PossiblyStale => "possibly stale",
            Self::Failing => "failing",
            Self::NotApplicable => "not applicable",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Milestone {
    pub id: &'static str,
    pub label: &'static str,
    pub state: State,
    pub detail: Option<String>,
}

impl Milestone {
    pub fn as_text(&self) -> String {
        match &self.detail {
            Some(detail) => format!("- {}: {} ({detail})", self.label, self.state.as_text()),
            None => format!("- {}: {}", self.label, self.state.as_text()),
        }
    }
}

const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "mov", "webm", "m4v", "wav", "flac", "m4a", "mp3", "aac", "ogg",
];
const TRANSCRIPT_FORMATS: &[&str] = &["srt", "vtt", "txt", "json", "tsv"];

/// Derive the channel-neutral milestones, in production order.
pub fn derive(
    episode_path: &Path,
    base: &str,
    stage: &str,
    fields: &EpisodeFields,
    artefacts: &[Artefact],
    findings: &[Finding],
) -> Vec<Milestone> {
    let file = |suffix: &str| episode_path.join(format!("{base}{suffix}"));
    let exists = |kind: &str| {
        artefacts
            .iter()
            .any(|item| item.kind == kind && item.exists)
    };
    // Media can be removed by retention once an episode is out; that is the
    // retention policy working, not missing work.
    let retired = stage == "published" && fields.video_id.is_some();

    let mut milestones = Vec::new();

    let mut missing = Vec::new();
    if fields.title.is_none() {
        missing.push("title");
    }
    if fields.episode_date.is_none() {
        missing.push("episode date");
    }
    if !exists("show_notes") {
        missing.push("show notes");
    }
    milestones.push(if missing.is_empty() {
        milestone("planned", "Planned", State::Present, None)
    } else {
        milestone(
            "planned",
            "Planned",
            State::Failing,
            Some(format!("missing {}", missing.join(", "))),
        )
    });

    let sources = source_media(episode_path, base);
    milestones.push(if !sources.is_empty() {
        let count = sources.len();
        milestone(
            "recorded",
            "Recorded",
            State::Present,
            Some(format!(
                "{count} source media file{}",
                if count == 1 { "" } else { "s" }
            )),
        )
    } else if exists("edited_video") || exists("upload_video") {
        milestone(
            "recorded",
            "Recorded",
            State::Present,
            Some("implied by an edited master".to_owned()),
        )
    } else if retired {
        retired_milestone("recorded", "Recorded")
    } else {
        milestone("recorded", "Recorded", State::NotStarted, None)
    });

    milestones.push(if exists("edited_video") {
        milestone("edited_master", "Edited master", State::Present, None)
    } else if exists("upload_video") {
        milestone(
            "edited_master",
            "Edited master",
            State::NotApplicable,
            Some("upload master present".to_owned()),
        )
    } else if retired {
        retired_milestone("edited_master", "Edited master")
    } else {
        milestone("edited_master", "Edited master", State::NotStarted, None)
    });

    milestones.push(if exists("upload_video") {
        derived(
            "upload_master",
            "Upload master",
            &file(".processed.mp4"),
            &[(&file(".mp4"), "edited master")],
        )
    } else if retired {
        retired_milestone("upload_master", "Upload master")
    } else {
        milestone("upload_master", "Upload master", State::NotStarted, None)
    });

    let present_formats = TRANSCRIPT_FORMATS
        .iter()
        .filter(|format| file(&format!(".{format}")).is_file())
        .collect::<Vec<_>>();
    let source = if file(".processed.mp4").is_file() {
        (file(".processed.mp4"), "upload master")
    } else {
        (file(".mp4"), "edited master")
    };
    milestones.push(if present_formats.is_empty() {
        milestone("transcript", "Transcript family", State::NotStarted, None)
    } else if present_formats.len() < TRANSCRIPT_FORMATS.len() {
        let absent = TRANSCRIPT_FORMATS
            .iter()
            .filter(|format| !present_formats.contains(format))
            .copied()
            .collect::<Vec<_>>();
        milestone(
            "transcript",
            "Transcript family",
            State::Failing,
            Some(format!(
                "{} of {} formats; missing {}",
                present_formats.len(),
                TRANSCRIPT_FORMATS.len(),
                absent.join(", ")
            )),
        )
    } else {
        checked(
            derived(
                "transcript",
                "Transcript family",
                &file(".srt"),
                &[(&source.0, source.1)],
            ),
            findings,
            "captions",
        )
    });

    let transcript = (file(".srt"), "transcript");
    for (id, label, kind, suffix, inputs, topic) in [
        (
            "chapters",
            "Chapters",
            "chapters",
            ".chapters.txt",
            vec![transcript.clone()],
            "chapter",
        ),
        (
            "tags",
            "Tags",
            "tags",
            ".tags.txt",
            vec![transcript.clone()],
            "tag",
        ),
        (
            "description",
            "Description",
            "description",
            ".youtube-description.txt",
            vec![(file(".chapters.txt"), "chapters"), transcript.clone()],
            "description",
        ),
    ] {
        milestones.push(if exists(kind) {
            let inputs = inputs
                .iter()
                .map(|(path, name)| (path.as_path(), *name))
                .collect::<Vec<_>>();
            checked(derived(id, label, &file(suffix), &inputs), findings, topic)
        } else {
            milestone(id, label, State::NotStarted, None)
        });
    }

    milestones.push(if exists("thumbnail") {
        checked(
            milestone("thumbnail", "Thumbnail", State::Present, None),
            findings,
            "thumbnail",
        )
    } else {
        let frames = frame_count(&file(".frames"));
        milestone(
            "thumbnail",
            "Thumbnail",
            State::NotStarted,
            (frames > 0).then(|| format!("{frames} frame candidates")),
        )
    });

    milestones.push(match &fields.video_id {
        Some(id) => milestone(
            "uploaded",
            "Uploaded",
            State::Present,
            Some(format!("video {id}")),
        ),
        None => milestone("uploaded", "Uploaded", State::NotStarted, None),
    });

    let public = fields
        .privacy_status
        .as_deref()
        .is_some_and(|privacy| privacy.eq_ignore_ascii_case("public"));
    milestones.push(match (public, stage == "published") {
        (true, true) => milestone("public", "Public", State::Present, None),
        (true, false) => milestone(
            "public",
            "Public",
            State::Failing,
            Some(format!("recorded as public, but the folder is in {stage}")),
        ),
        (false, true) => milestone(
            "public",
            "Public",
            State::Failing,
            Some(format!(
                "folder is in published, but recorded privacy is {}",
                fields.privacy_status.as_deref().unwrap_or("not set")
            )),
        ),
        (false, false) => milestone("public", "Public", State::NotStarted, None),
    });

    milestones
}

fn milestone(
    id: &'static str,
    label: &'static str,
    state: State,
    detail: Option<String>,
) -> Milestone {
    Milestone {
        id,
        label,
        state,
        detail,
    }
}

fn retired_milestone(id: &'static str, label: &'static str) -> Milestone {
    milestone(
        id,
        label,
        State::NotApplicable,
        Some("not required after publication".to_owned()),
    )
}

/// Present, or possibly stale when the artefact is older than any input it
/// was derived from. Modification times can be changed by a filesystem
/// synchroniser, so this is a prompt to look, not a verdict.
fn derived(
    id: &'static str,
    label: &'static str,
    artefact: &Path,
    inputs: &[(&Path, &str)],
) -> Milestone {
    let Some(made) = modified(artefact) else {
        return milestone(id, label, State::Present, None);
    };
    let newer = inputs
        .iter()
        .filter(|(input, _)| modified(input).is_some_and(|time| time > made))
        .map(|(_, name)| *name)
        .collect::<Vec<_>>();
    if newer.is_empty() {
        milestone(id, label, State::Present, None)
    } else {
        milestone(
            id,
            label,
            State::PossiblyStale,
            Some(format!("older than the {}", newer.join(" and "))),
        )
    }
}

/// Inspection findings about an artefact that exists override its presence.
/// Findings about absence are already expressed by the milestone state.
fn checked(milestone: Milestone, findings: &[Finding], topic: &str) -> Milestone {
    let problems = findings
        .iter()
        .filter(|finding| finding.code.contains(topic) && !finding.code.starts_with("missing-"))
        .map(|finding| finding.message.clone())
        .collect::<Vec<_>>();
    if problems.is_empty() {
        milestone
    } else {
        Milestone {
            state: State::Failing,
            detail: Some(problems.join("; ")),
            ..milestone
        }
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|meta| meta.modified()).ok()
}

/// Media in the episode folder other than the canonical masters and the
/// standardised transcription track.
fn source_media(episode_path: &Path, base: &str) -> Vec<String> {
    let canonical = [
        format!("{base}.mp4"),
        format!("{base}.processed.mp4"),
        format!("{base}.wav"),
        format!("{base}.processed.wav"),
    ];
    let Ok(entries) = fs::read_dir(episode_path) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.') && !canonical.contains(name))
        .filter(|name| {
            Path::new(name)
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    MEDIA_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
                })
        })
        .collect()
}

fn frame_count(directory: &Path) -> usize {
    fs::read_dir(directory)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| is_frame_name(&entry.file_name().to_string_lossy()))
                .count()
        })
        .unwrap_or(0)
}

/// A frame candidate file name: a plain image name, never a path.
pub fn is_frame_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !name.starts_with('.')
        && name.len() <= 128
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        && (lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".png"))
}

/// Frame candidates for an episode, sorted by name.
pub fn frames(episode_path: &Path, base: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir(episode_path.join(format!("{base}.frames"))) else {
        return Vec::new();
    };
    let mut names = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| is_frame_name(name))
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::episode;
    use std::time::Duration;

    const BASE: &str = "2026.11.23-example";

    fn episode_in(stage: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join(stage).join(BASE);
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join(format!("{BASE}.episode-data.md")),
            "- **Title**: Example\n- **Episode Date**: 2026-11-23\n",
        )
        .unwrap();
        fs::write(folder.join(format!("{BASE}.md")), "notes").unwrap();
        (temp, folder)
    }

    fn state(folder: &Path, id: &str) -> (State, Option<String>) {
        let report = episode::inspect(folder).unwrap();
        let found = report
            .milestones
            .into_iter()
            .find(|milestone| milestone.id == id)
            .unwrap();
        (found.state, found.detail)
    }

    fn age(path: &Path, seconds_ago: u64) {
        let file = fs::File::options().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(seconds_ago))
            .unwrap();
    }

    #[test]
    fn a_planned_episode_is_planned_and_nothing_else_has_started() {
        let (_temp, folder) = episode_in("in-planning");
        let report = episode::inspect(&folder).unwrap();
        assert_eq!(report.milestones.len(), 11);
        assert_eq!(report.milestones[0].state, State::Present);
        assert!(
            report.milestones[1..]
                .iter()
                .all(|milestone| milestone.state == State::NotStarted)
        );
    }

    #[test]
    fn a_partial_transcript_family_is_failing_and_names_what_is_missing() {
        let (_temp, folder) = episode_in("in-progress");
        for format in ["srt", "txt"] {
            fs::write(folder.join(format!("{BASE}.{format}")), "x").unwrap();
        }
        let (state, detail) = state(&folder, "transcript");
        assert_eq!(state, State::Failing);
        assert_eq!(detail.unwrap(), "2 of 5 formats; missing vtt, json, tsv");
    }

    #[test]
    fn an_artefact_older_than_its_input_is_possibly_stale() {
        let (_temp, folder) = episode_in("in-progress");
        let srt = folder.join(format!("{BASE}.srt"));
        let chapters = folder.join(format!("{BASE}.chapters.txt"));
        fs::write(&srt, "1\n00:00:00,000 --> 00:00:10,000\nHello\n").unwrap();
        fs::write(&chapters, "0:00 Introduction\n0:09 Close\n").unwrap();
        age(&chapters, 3600);
        let (state, detail) = state(&folder, "chapters");
        assert_eq!(state, State::PossiblyStale);
        assert_eq!(detail.unwrap(), "older than the transcript");
    }

    #[test]
    fn source_recordings_count_but_canonical_masters_do_not() {
        let (_temp, folder) = episode_in("in-progress");
        fs::write(folder.join(format!("{BASE}.wav")), "x").unwrap();
        assert_eq!(state(&folder, "recorded").0, State::NotStarted);
        fs::write(folder.join("obs-recording.mkv"), "x").unwrap();
        assert_eq!(
            state(&folder, "recorded"),
            (State::Present, Some("1 source media file".to_owned()))
        );
    }

    #[test]
    fn published_media_removed_by_retention_is_not_applicable() {
        let (_temp, folder) = episode_in("published");
        fs::write(
            folder.join(format!("{BASE}.episode-data.md")),
            "- **Title**: Example\n- **Episode Date**: 2026-11-23\n- **Video ID**: dQw4w9WgXcQ\n- **Privacy Status**: public\n",
        )
        .unwrap();
        assert_eq!(state(&folder, "edited_master").0, State::NotApplicable);
        assert_eq!(state(&folder, "public").0, State::Present);
    }

    #[test]
    fn frame_names_never_carry_a_path() {
        assert!(is_frame_name("frame-03.jpg"));
        for name in ["../secret.jpg", "a/b.png", ".hidden.jpg", "frame.gif", ""] {
            assert!(!is_frame_name(name), "{name}");
        }
    }
}
