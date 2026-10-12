// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Conservative media-retention planning. This module only identifies and
//! quarantines known disposable or reviewable artefacts; it never deletes.

use crate::episode;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const TRASH_DIRECTORY: &str = ".episode-cleanup-trash";
const CACHE_DIRECTORIES: &[&str] = &["cachefiles", "audiothumbs", "videothumbs", "BL_proxy"];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CandidateClass {
    Safe,
    Review,
}

#[derive(Debug, Serialize)]
pub struct Candidate {
    pub episode_path: PathBuf,
    pub episode_date: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub class: CandidateClass,
    pub reason: &'static str,
    pub selected: bool,
}

#[derive(Debug, Serialize)]
pub struct AuditReport {
    pub episodes_path: PathBuf,
    pub retention_days: u32,
    pub stages: Vec<String>,
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Serialize)]
pub struct CleanupReport {
    pub episode_path: PathBuf,
    pub candidates: Vec<Candidate>,
    pub applied: bool,
    pub quarantine_path: Option<PathBuf>,
}

impl AuditReport {
    pub fn as_text(&self) -> String {
        if self.candidates.is_empty() {
            return "No retention candidates found".to_owned();
        }
        let mut lines = vec![format!(
            "Retention candidates older than {} days:",
            self.retention_days
        )];
        lines.extend(self.candidates.iter().map(candidate_text));
        lines.push(format!("Total: {} bytes", total_bytes(&self.candidates)));
        lines.join("\n")
    }
}

impl CleanupReport {
    pub fn as_text(&self) -> String {
        let heading = if self.applied {
            "Quarantined"
        } else {
            "Would quarantine"
        };
        let mut lines = vec![format!("{heading} {} candidate(s)", self.candidates.len())];
        lines.extend(self.candidates.iter().map(candidate_text));
        if let Some(path) = &self.quarantine_path {
            lines.push(format!("Quarantine: {}", path.display()));
        }
        lines.push(format!("Total: {} bytes", total_bytes(&self.candidates)));
        lines.join("\n")
    }
}

pub fn audit(episodes_path: &Path, stages: &[String], retention_days: u32) -> Result<AuditReport> {
    if retention_days == 0 {
        bail!("storage.raw_media_retention_days must be greater than zero");
    }
    let stages = if stages.is_empty() {
        vec!["published".to_owned()]
    } else {
        stages.to_vec()
    };
    let today = current_day()?;
    let mut candidates = Vec::new();
    for stage in &stages {
        for episode_path in episode_directories(&episodes_path.join(stage))? {
            let Some((date, day)) = episode_date(&episode_path) else {
                continue;
            };
            if today.saturating_sub(day) < i64::from(retention_days) {
                continue;
            }
            candidates.extend(candidates_for_episode(&episode_path, &date, false)?);
        }
    }
    candidates.sort_by(candidate_order);
    Ok(AuditReport {
        episodes_path: episodes_path.to_path_buf(),
        retention_days,
        stages,
        candidates,
    })
}

pub fn cleanup_plan(episode_path: &Path, include_review: bool) -> Result<CleanupReport> {
    let (_, metadata_path, _) = episode::locate(episode_path)?;
    let episode_path = metadata_path
        .parent()
        .context("episode metadata has no parent directory")?
        .to_path_buf();
    let date = episode_date(&episode_path)
        .map(|(date, _)| date)
        .context("episode directory must start YYYY.MM.DD-")?;
    let candidates = candidates_for_episode(&episode_path, &date, include_review)?;
    Ok(CleanupReport {
        episode_path,
        candidates,
        applied: false,
        quarantine_path: None,
    })
}

pub fn apply_cleanup(mut plan: CleanupReport) -> Result<CleanupReport> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_secs();
    let quarantine_path = plan
        .episode_path
        .join(TRASH_DIRECTORY)
        .join(stamp.to_string());
    if quarantine_path.exists() {
        bail!(
            "quarantine destination already exists: {}",
            quarantine_path.display()
        );
    }
    for candidate in plan
        .candidates
        .iter()
        .filter(|candidate| candidate.selected)
    {
        let relative = candidate
            .path
            .strip_prefix(&plan.episode_path)
            .with_context(|| {
                format!(
                    "candidate is outside episode directory: {}",
                    candidate.path.display()
                )
            })?;
        let destination = quarantine_path.join(relative);
        if destination.exists() {
            bail!(
                "quarantine destination already exists: {}",
                destination.display()
            );
        }
    }
    for candidate in plan
        .candidates
        .iter()
        .filter(|candidate| candidate.selected)
    {
        let relative = candidate.path.strip_prefix(&plan.episode_path).unwrap();
        let destination = quarantine_path.join(relative);
        fs::create_dir_all(destination.parent().unwrap())?;
        fs::rename(&candidate.path, &destination).with_context(|| {
            format!(
                "could not move {} to {}",
                candidate.path.display(),
                destination.display()
            )
        })?;
    }
    plan.applied = true;
    plan.quarantine_path = Some(quarantine_path);
    Ok(plan)
}

fn candidates_for_episode(
    episode_path: &Path,
    episode_date: &str,
    include_review: bool,
) -> Result<Vec<Candidate>> {
    let (_, _, base) = episode::locate(episode_path)?;
    let fields = episode::inspect(episode_path)?.fields;
    let protected = protected_videos(episode_path, &base, fields.uploaded_file.as_deref());
    let mut candidates = Vec::new();
    visit(
        episode_path,
        episode_path,
        episode_date,
        &base,
        &protected,
        include_review,
        &mut candidates,
    )?;
    candidates.sort_by(candidate_order);
    Ok(candidates)
}

fn visit(
    root: &Path,
    directory: &Path,
    episode_date: &str,
    base: &str,
    protected: &[PathBuf],
    include_review: bool,
    candidates: &mut Vec<Candidate>,
) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if name == TRASH_DIRECTORY {
            continue;
        }
        if path.is_dir() {
            if CACHE_DIRECTORIES.contains(&name)
                || name.ends_with(".frames")
                || name == "thumbnail-options"
            {
                push_candidate(
                    root,
                    episode_date,
                    path,
                    CandidateClass::Safe,
                    "generated-cache-or-thumbnail-candidates",
                    true,
                    candidates,
                )?;
            } else {
                visit(
                    root,
                    &path,
                    episode_date,
                    base,
                    protected,
                    include_review,
                    candidates,
                )?;
            }
            continue;
        }
        if protected
            .iter()
            .any(|protected_path| protected_path == &path)
        {
            continue;
        }
        let Some((class, reason)) = classify_file(&path, base, protected) else {
            continue;
        };
        push_candidate(
            root,
            episode_date,
            path,
            class,
            reason,
            class == CandidateClass::Safe || include_review,
            candidates,
        )?;
    }
    Ok(())
}

fn push_candidate(
    root: &Path,
    episode_date: &str,
    path: PathBuf,
    class: CandidateClass,
    reason: &'static str,
    selected: bool,
    candidates: &mut Vec<Candidate>,
) -> Result<()> {
    candidates.push(Candidate {
        episode_path: root.to_path_buf(),
        episode_date: episode_date.to_owned(),
        bytes: path_size(&path)?,
        path,
        class,
        reason,
        selected,
    });
    Ok(())
}

fn classify_file(
    path: &Path,
    base: &str,
    protected: &[PathBuf],
) -> Option<(CandidateClass, &'static str)> {
    let name = path.file_name()?.to_str()?;
    let lower = name.to_ascii_lowercase();
    let suffix = path.extension()?.to_str()?.to_ascii_lowercase();
    if protected
        .iter()
        .any(|protected_path| protected_path == path)
    {
        return None;
    }
    if matches!(
        name,
        _ if name == format!("{base}.bluesky-draft.txt")
            || name == format!("{base}.mastodon-draft.txt")
            || name == format!("{base}.linkedin-draft.txt")
            || name == format!("{base}.linkedin-comment.txt")
            || name == format!("{base}.instagram-caption.txt")
    ) {
        return Some((CandidateClass::Safe, "legacy-social-draft-sidecar"));
    }
    match suffix.as_str() {
        "blend1" => Some((CandidateClass::Safe, "blender-autosave")),
        "blend" => Some((CandidateClass::Review, "blender-project")),
        "wav" | "flac" | "mp3" => Some((CandidateClass::Review, "intermediate-audio")),
        "mkv" | "mov" | "avi" => Some((CandidateClass::Review, "raw-or-editor-video")),
        "mp4" if name == format!("{base}.mp4") => Some((
            CandidateClass::Review,
            "superseded-pre-audio-processed-video",
        )),
        "mp4" if lower.ends_with("-auphonic.mp4") => {
            Some((CandidateClass::Review, "legacy-auphonic-render"))
        }
        "mp4" if lower.contains("-30fps") => Some((CandidateClass::Review, "derived-30fps-render")),
        "mp4" if lower.contains("converted") => {
            Some((CandidateClass::Review, "converted-video-render"))
        }
        "mp4" if lower.contains("repaired") => {
            Some((CandidateClass::Review, "repaired-video-render"))
        }
        "mp4" if lower.starts_with("test") => Some((CandidateClass::Review, "test-video-render")),
        "mp4" => Some((CandidateClass::Review, "non-canonical-video")),
        _ => None,
    }
}

fn protected_videos(episode_path: &Path, base: &str, uploaded_file: Option<&str>) -> Vec<PathBuf> {
    let recorded = uploaded_file.and_then(|value| {
        let path = Path::new(value);
        (!path.is_absolute()
            && path
                .components()
                .all(|component| !matches!(component, std::path::Component::ParentDir)))
        .then(|| episode_path.join(path))
        .filter(|path| path.is_file())
    });
    if let Some(path) = recorded {
        return vec![path];
    }
    let processed = episode_path.join(format!("{base}.processed.mp4"));
    if processed.is_file() {
        vec![processed]
    } else {
        vec![episode_path.join(format!("{base}.mp4"))]
    }
}

fn episode_directories(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut directories = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if !path.is_dir() {
            continue;
        }
        if episode_date(&path).is_some() {
            directories.push(path);
        } else {
            directories.extend(episode_directories(&path)?);
        }
    }
    Ok(directories)
}

fn episode_date(path: &Path) -> Option<(String, i64)> {
    let name = path.file_name()?.to_str()?;
    let date = name.get(..10)?;
    let [year, month, day] = date
        .split('.')
        .map(str::parse::<i32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?
        .try_into()
        .ok()?;
    (name.as_bytes().get(10) == Some(&b'-') && valid_date(year, month, day))
        .then(|| (date.to_owned(), days_from_civil(year, month, day)))
}

fn current_day() -> Result<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_secs() as i64
        / 86_400)
}

fn valid_date(year: i32, month: i32, day: i32) -> bool {
    (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month)
}

fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 31,
    }
}

fn days_from_civil(year: i32, month: i32, day: i32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    i64::from(era * 146_097 + doe - 719_468)
}

fn path_size(path: &Path) -> Result<u64> {
    if path.is_file() {
        return Ok(fs::metadata(path)?.len());
    }
    let mut total = 0;
    for entry in fs::read_dir(path)? {
        total += path_size(&entry?.path())?;
    }
    Ok(total)
}

fn candidate_order(left: &Candidate, right: &Candidate) -> std::cmp::Ordering {
    left.class
        .cmp(&right.class)
        .then_with(|| right.bytes.cmp(&left.bytes))
        .then_with(|| left.path.cmp(&right.path))
}

fn candidate_text(candidate: &Candidate) -> String {
    format!(
        "- {} [{} {}]: {}",
        candidate.path.display(),
        match candidate.class {
            CandidateClass::Safe => "safe",
            CandidateClass::Review => "review",
        },
        candidate.reason,
        candidate.bytes
    )
}

fn total_bytes(candidates: &[Candidate]) -> u64 {
    candidates.iter().map(|candidate| candidate.bytes).sum()
}
