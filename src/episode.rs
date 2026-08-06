// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const MIN_CHAPTER_COVERAGE: f64 = 0.8;

#[derive(Debug, Serialize)]
pub struct EpisodeReport {
    pub episode_path: PathBuf,
    pub metadata_path: PathBuf,
    pub base: String,
    pub stage: String,
    pub fields: EpisodeFields,
    pub artefacts: Vec<Artefact>,
    pub ready_for_upload: bool,
    pub findings: Vec<Finding>,
    pub next_action: String,
}

#[derive(Debug, Default, Serialize)]
pub struct EpisodeFields {
    pub episode_date: Option<String>,
    pub publication_date: Option<String>,
    pub title: Option<String>,
    pub guest: Option<String>,
    pub duration: Option<String>,
    pub video_id: Option<String>,
    pub youtube_url: Option<String>,
    pub privacy_status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Artefact {
    pub kind: &'static str,
    pub path: PathBuf,
    pub exists: bool,
    pub bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
}

impl EpisodeReport {
    pub fn is_valid(&self) -> bool {
        !self
            .findings
            .iter()
            .any(|finding| finding.severity == Severity::Error)
    }

    pub fn as_text(&self) -> String {
        let title = self.fields.title.as_deref().unwrap_or("Untitled episode");
        let mut lines = vec![
            format!("Episode: {title}"),
            format!("Path: {}", self.episode_path.display()),
            format!("Stage: {}", self.stage),
            format!(
                "Ready for upload: {}",
                if self.ready_for_upload { "yes" } else { "no" }
            ),
            "Artefacts:".to_owned(),
        ];
        lines.extend(self.artefacts.iter().map(|artefact| {
            let status = if artefact.exists {
                "present"
            } else {
                "missing"
            };
            format!(
                "- {}: {status} ({})",
                artefact.kind,
                artefact.path.display()
            )
        }));
        if self.findings.is_empty() {
            lines.push("Findings: none".to_owned());
        } else {
            lines.push("Findings:".to_owned());
            lines.extend(self.findings.iter().map(|finding| {
                format!(
                    "- {} [{}]: {}",
                    match finding.severity {
                        Severity::Error => "ERROR",
                        Severity::Warning => "WARNING",
                    },
                    finding.code,
                    finding.message
                )
            }));
        }
        lines.push(format!("Next action: {}", self.next_action));
        lines.join("\n")
    }
}

pub fn inspect(input: &Path) -> Result<EpisodeReport> {
    let (episode_path, metadata_path, base) = locate(input)?;
    let markdown = fs::read_to_string(&metadata_path).with_context(|| {
        format!(
            "could not read episode metadata {}",
            metadata_path.display()
        )
    })?;
    let values = labelled_fields(&markdown);
    let fields = EpisodeFields {
        episode_date: value(&values, "Episode Date"),
        publication_date: value(&values, "Publication Date"),
        title: value(&values, "Title"),
        guest: value(&values, "Guest"),
        duration: value(&values, "Duration"),
        video_id: value(&values, "Video ID"),
        youtube_url: value(&values, "YouTube URL"),
        privacy_status: value(&values, "Privacy Status"),
    };
    let stage = lifecycle_stage(&episode_path);
    let artefacts = artefacts(&episode_path, &base);
    let mut findings = validate_local_state(&episode_path, &base, &fields, &artefacts);
    validate_lifecycle(&stage, &fields, &mut findings);
    let ready_for_upload = !findings
        .iter()
        .any(|finding| finding.severity == Severity::Error);
    let next_action = next_action(&stage, &fields, ready_for_upload);

    Ok(EpisodeReport {
        episode_path,
        metadata_path,
        base,
        stage,
        fields,
        artefacts,
        ready_for_upload,
        findings,
        next_action,
    })
}

/// Resolve an episode directory or metadata path to
/// `(episode directory, metadata file, base name)`.
pub fn locate(input: &Path) -> Result<(PathBuf, PathBuf, String)> {
    if input.is_file() {
        let filename = input
            .file_name()
            .and_then(|name| name.to_str())
            .context("episode metadata filename is not valid UTF-8")?;
        let base = filename
            .strip_suffix(".episode-data.md")
            .context("episode metadata file must end with .episode-data.md")?;
        let episode_path = input
            .parent()
            .context("episode metadata has no parent directory")?
            .to_path_buf();
        return Ok((episode_path, input.to_path_buf(), base.to_owned()));
    }
    if !input.is_dir() {
        bail!("episode path not found: {}", input.display());
    }
    let base = input
        .file_name()
        .and_then(|name| name.to_str())
        .context("episode directory name is not valid UTF-8")?
        .to_owned();
    let expected = input.join(format!("{base}.episode-data.md"));
    if expected.is_file() {
        return Ok((input.to_path_buf(), expected, base));
    }

    let candidates = fs::read_dir(input)?
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".episode-data.md"))
        })
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [metadata] => {
            let metadata_name = metadata.file_name().unwrap().to_string_lossy();
            let metadata_base = metadata_name
                .trim_end_matches(".episode-data.md")
                .to_owned();
            Ok((input.to_path_buf(), metadata.clone(), metadata_base))
        }
        [] => bail!("episode metadata not found in {}", input.display()),
        _ => bail!(
            "multiple episode metadata files found in {}",
            input.display()
        ),
    }
}

fn labelled_fields(markdown: &str) -> BTreeMap<String, String> {
    markdown
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("- **")?;
            let (label, raw_value) = rest.split_once("**:")?;
            Some((label.trim().to_owned(), raw_value.trim().to_owned()))
        })
        .collect()
}

fn value(fields: &BTreeMap<String, String>, label: &str) -> Option<String> {
    let value = fields.get(label)?.trim();
    if value.is_empty() || value.starts_with('[') {
        None
    } else {
        Some(value.trim_matches('`').to_owned())
    }
}

fn lifecycle_stage(episode_path: &Path) -> String {
    episode_path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .filter(|name| {
            matches!(
                *name,
                "in-planning" | "in-progress" | "edited" | "published" | "not-for-publishing"
            )
        })
        .unwrap_or("unknown")
        .to_owned()
}

fn artefacts(episode_path: &Path, base: &str) -> Vec<Artefact> {
    [
        ("metadata", format!("{base}.episode-data.md")),
        ("show_notes", format!("{base}.md")),
        ("edited_video", format!("{base}.mp4")),
        ("upload_video", format!("{base}.processed.mp4")),
        ("captions", format!("{base}.srt")),
        ("chapters", format!("{base}.chapters.txt")),
        ("tags", format!("{base}.tags.txt")),
        ("description", format!("{base}.youtube-description.txt")),
        ("thumbnail", format!("{base}.thumbnail.png")),
    ]
    .into_iter()
    .map(|(kind, filename)| {
        let path = episode_path.join(filename);
        let metadata = fs::metadata(&path).ok();
        Artefact {
            kind,
            exists: metadata.is_some(),
            bytes: metadata.map(|item| item.len()),
            path,
        }
    })
    .collect()
}

fn validate_local_state(
    episode_path: &Path,
    base: &str,
    fields: &EpisodeFields,
    artefacts: &[Artefact],
) -> Vec<Finding> {
    let mut findings = Vec::new();
    if fields.title.is_none() {
        error(&mut findings, "missing-title", "Episode title is missing");
    }
    if fields.episode_date.is_none() {
        error(&mut findings, "missing-date", "Episode date is missing");
    }

    let edited = artefact(artefacts, "edited_video").exists;
    let processed = artefact(artefacts, "upload_video").exists;
    if !edited && !processed {
        error(
            &mut findings,
            "missing-video",
            "No edited or processed MP4 is present",
        );
    } else if !processed {
        warning(
            &mut findings,
            "missing-processed-video",
            "No processed upload master is present; upload would fall back to the edited master",
        );
    }

    validate_description(
        episode_path.join(format!("{base}.youtube-description.txt")),
        &mut findings,
    );
    validate_tags(episode_path.join(format!("{base}.tags.txt")), &mut findings);
    validate_thumbnail(
        episode_path.join(format!("{base}.thumbnail.png")),
        &mut findings,
    );
    validate_chapter_coverage(
        episode_path.join(format!("{base}.srt")),
        episode_path.join(format!("{base}.chapters.txt")),
        &mut findings,
    );
    findings
}

fn validate_description(path: PathBuf, findings: &mut Vec<Finding>) {
    let Ok(description) = fs::read_to_string(&path) else {
        error(
            findings,
            "missing-description",
            "YouTube description is missing",
        );
        return;
    };
    if description.chars().count() > 5000 {
        error(
            findings,
            "description-too-long",
            "YouTube description exceeds 5000 characters",
        );
    }
    for heading in ["CHAPTERS", "LINKS MENTIONED"] {
        if !description.lines().any(|line| line.trim() == heading) {
            error(
                findings,
                "description-heading-missing",
                format!("YouTube description is missing the {heading} heading"),
            );
        }
    }
}

fn validate_tags(path: PathBuf, findings: &mut Vec<Finding>) {
    let Ok(tags) = fs::read_to_string(path) else {
        error(findings, "missing-tags", "Tag list is missing");
        return;
    };
    let tags = tags.trim();
    let count = tags.split(',').filter(|tag| !tag.trim().is_empty()).count();
    if !(10..=15).contains(&count) {
        error(
            findings,
            "invalid-tag-count",
            format!("Tag list contains {count} tags; expected 10-15"),
        );
    }
    if tags.chars().count() > 500 {
        error(findings, "tags-too-long", "Tag list exceeds 500 characters");
    }
    if tags.contains(", ") {
        warning(
            findings,
            "tag-spacing",
            "Tag list contains spaces after commas",
        );
    }
}

fn validate_thumbnail(path: PathBuf, findings: &mut Vec<Finding>) {
    if !path.is_file() {
        warning(findings, "missing-thumbnail", "Thumbnail is missing");
        return;
    }
    match png_dimensions(&path) {
        Ok((1280, 720)) => {}
        Ok((width, height)) => error(
            findings,
            "thumbnail-dimensions",
            format!("Thumbnail is {width}x{height}; expected 1280x720"),
        ),
        Err(error_message) => error(findings, "invalid-thumbnail", error_message),
    }
}

fn png_dimensions(path: &Path) -> Result<(u32, u32), String> {
    let bytes = fs::read(path).map_err(|error| format!("Could not read thumbnail: {error}"))?;
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return Err("Thumbnail is not a readable PNG".to_owned());
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    Ok((width, height))
}

fn validate_chapter_coverage(
    srt_path: PathBuf,
    chapters_path: PathBuf,
    findings: &mut Vec<Finding>,
) {
    if !srt_path.is_file() {
        warning(findings, "missing-captions", "SRT captions are missing");
        return;
    }
    if !chapters_path.is_file() {
        warning(findings, "missing-chapters", "Chapter markers are missing");
        return;
    }
    let Ok(srt) = fs::read_to_string(srt_path) else {
        error(findings, "invalid-captions", "Could not read SRT captions");
        return;
    };
    let Ok(chapters) = fs::read_to_string(chapters_path) else {
        error(
            findings,
            "invalid-chapters",
            "Could not read chapter markers",
        );
        return;
    };
    let transcript_end = srt
        .lines()
        .filter_map(|line| line.split_once("-->").map(|(_, end)| end.trim()))
        .filter_map(parse_srt_timestamp)
        .next_back();
    let final_chapter = chapters
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter_map(parse_chapter_timestamp)
        .next_back();
    match (final_chapter, transcript_end) {
        (Some(chapter), Some(end)) if end > 0.0 && chapter / end < MIN_CHAPTER_COVERAGE => error(
            findings,
            "chapter-coverage",
            format!(
                "Final chapter starts at {:.0}% of the transcript; expected at least {:.0}%",
                chapter / end * 100.0,
                MIN_CHAPTER_COVERAGE * 100.0
            ),
        ),
        (Some(_), Some(_)) => {}
        _ => error(
            findings,
            "chapter-timestamps",
            "Could not compare chapter markers with the transcript end",
        ),
    }
}

fn parse_srt_timestamp(timestamp: &str) -> Option<f64> {
    let timestamp = timestamp.split_whitespace().next()?;
    let (clock, millis) = timestamp.split_once([',', '.'])?;
    Some(parse_clock(clock)? as f64 + millis.parse::<f64>().ok()? / 1000.0)
}

fn parse_chapter_timestamp(timestamp: &str) -> Option<f64> {
    Some(parse_clock(timestamp)? as f64)
}

fn parse_clock(clock: &str) -> Option<u64> {
    let parts = clock
        .split(':')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    match parts.as_slice() {
        [minutes, seconds] if *seconds < 60 => Some(minutes * 60 + seconds),
        [hours, minutes, seconds] if *minutes < 60 && *seconds < 60 => {
            Some(hours * 3600 + minutes * 60 + seconds)
        }
        _ => None,
    }
}

fn validate_lifecycle(stage: &str, fields: &EpisodeFields, findings: &mut Vec<Finding>) {
    let privacy = fields
        .privacy_status
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if stage == "published" && !privacy.starts_with("public") {
        warning(
            findings,
            "published-privacy-mismatch",
            "Episode is in published but local privacy status is not public",
        );
    }
    if privacy.starts_with("public") && stage != "published" {
        warning(
            findings,
            "public-stage-mismatch",
            format!("Remote status is recorded as public but local stage is {stage}"),
        );
    }
    let solo = fields
        .guest
        .as_deref()
        .is_some_and(|guest| guest.to_ascii_lowercase().contains("solo episode"));
    if solo && privacy.contains("awaiting guest review") {
        warning(
            findings,
            "solo-guest-review",
            "Solo episode is marked as awaiting guest review",
        );
    }
}

fn next_action(stage: &str, fields: &EpisodeFields, ready_for_upload: bool) -> String {
    if !ready_for_upload {
        return "Resolve validation errors".to_owned();
    }
    let privacy = fields
        .privacy_status
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if privacy.starts_with("public") && stage != "published" {
        return "Verify publication and promote the episode to published".to_owned();
    }
    if fields.video_id.is_none() {
        return "Plan an upload to the configured destination".to_owned();
    }
    if privacy.starts_with("unlisted") {
        return "Complete final checks before publishing".to_owned();
    }
    "No local action inferred".to_owned()
}

fn artefact<'a>(artefacts: &'a [Artefact], kind: &str) -> &'a Artefact {
    artefacts
        .iter()
        .find(|artefact| artefact.kind == kind)
        .unwrap()
}

fn error(findings: &mut Vec<Finding>, code: &'static str, message: impl Into<String>) {
    findings.push(Finding {
        severity: Severity::Error,
        code,
        message: message.into(),
    });
}

fn warning(findings: &mut Vec<Finding>, code: &'static str, message: impl Into<String>) {
    findings.push(Finding {
        severity: Severity::Warning,
        code,
        message: message.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::{labelled_fields, parse_chapter_timestamp, parse_srt_timestamp, value};

    #[test]
    fn parses_labelled_fields_and_placeholders() {
        let fields = labelled_fields(
            "- **Title**: Test episode\n- **Video ID**: [To be added]\n- **Show Notes**: `notes.md`\n",
        );
        assert_eq!(value(&fields, "Title").as_deref(), Some("Test episode"));
        assert_eq!(value(&fields, "Video ID"), None);
        assert_eq!(value(&fields, "Show Notes").as_deref(), Some("notes.md"));
    }

    #[test]
    fn parses_supported_timestamps() {
        assert_eq!(parse_chapter_timestamp("24:51"), Some(1491.0));
        assert_eq!(parse_chapter_timestamp("1:02:03"), Some(3723.0));
        assert_eq!(parse_srt_timestamp("00:25:15,460"), Some(1515.46));
    }
}
