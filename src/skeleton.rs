// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Creation of a new episode folder and its starting documents.
//!
//! Every channel-specific value in the skeleton comes from channel
//! configuration. The shell scripts this replaces hardcoded Everything Digital
//! Health playlist IDs, archive URLs, and recording links, which is exactly
//! what stopped them serving a second channel.

use crate::channel::ChannelConfig;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const PLACEHOLDER: &str = "[To be added]";

#[derive(Debug, Serialize)]
pub struct NewEpisode {
    pub base: String,
    pub episode_path: PathBuf,
    pub files: Vec<NewFile>,
    pub applied: bool,
}

#[derive(Debug, Serialize)]
pub struct NewFile {
    pub path: PathBuf,
    #[serde(skip)]
    pub content: String,
    pub bytes: usize,
}

impl NewEpisode {
    pub fn as_text(&self) -> String {
        let heading = if self.applied {
            "Created"
        } else {
            "Would create"
        };
        let mut lines = vec![format!("{heading} {}", self.episode_path.display())];
        lines.extend(self.files.iter().map(|file| {
            format!(
                "  {} ({} bytes)",
                file.path.file_name().map_or_else(
                    || file.path.display().to_string(),
                    |name| name.to_string_lossy().into_owned()
                ),
                file.bytes
            )
        }));
        lines.join("\n")
    }
}

/// Plan a new episode folder in `in-planning`.
///
/// Nothing is written until [`create`] is called, so the plan is what
/// `--dry-run` reports and what an apply performs.
pub fn plan(
    episodes_path: &Path,
    date: &str,
    title: &str,
    guest: Option<&str>,
    config: &ChannelConfig,
) -> Result<NewEpisode> {
    let iso = normalise_date(date)?;
    let title = strip_leading_date(title);
    if title.is_empty() {
        bail!("title was only a date; give a real title too");
    }
    let slug = slugify(title);
    if slug.is_empty() {
        bail!("title '{title}' contains no characters usable in a folder name");
    }

    let base = format!("{}-{slug}", iso.replace('-', "."));
    let episode_path = episodes_path.join("in-planning").join(&base);
    if episode_path.exists() {
        bail!("{} already exists", episode_path.display());
    }

    let files = vec![
        file(
            episode_path.join(format!("{base}.md")),
            show_notes(title, &iso, config),
        ),
        file(
            episode_path.join(format!("{base}.episode-data.md")),
            episode_data(title, &iso, &base, guest, config),
        ),
    ];

    Ok(NewEpisode {
        base,
        episode_path,
        files,
        applied: false,
    })
}

/// Create the planned folder, staging it beside its destination so a failure
/// part way through leaves no half-built episode behind.
pub fn create(plan: &mut NewEpisode) -> Result<()> {
    let parent = plan
        .episode_path
        .parent()
        .expect("planned path has a lifecycle parent");
    fs::create_dir_all(parent)?;

    let staging = tempfile::Builder::new()
        .prefix(&format!(".{}.", plan.base))
        .tempdir_in(parent)?;
    for new in &plan.files {
        let name = new.path.file_name().expect("planned file has a name");
        fs::write(staging.path().join(name), &new.content)?;
    }

    let staged = staging.keep();
    if let Err(error) = fs::rename(&staged, &plan.episode_path) {
        let _ = fs::remove_dir_all(&staged);
        return Err(anyhow::Error::from(error))
            .with_context(|| format!("could not create {}", plan.episode_path.display()));
    }
    plan.applied = true;
    Ok(())
}

fn file(path: PathBuf, content: String) -> NewFile {
    NewFile {
        bytes: content.len(),
        path,
        content,
    }
}

fn show_notes(title: &str, iso: &str, config: &ChannelConfig) -> String {
    let mut sections = vec![
        format!("# {title}\n"),
        format!("## Date\n\n{}\n", iso.replace('-', ".")),
        format!(
            "## Presenter(s) and Guest(s)\n\n{}\n",
            some_or(&config.channel.host, PLACEHOLDER)
        ),
    ];

    if let Some(recording) = config.recording.values().next() {
        let mut links = vec!["## Recording Links\n".to_owned()];
        for (label, url) in [
            ("Guest Link", &recording.guest_url),
            ("Host Link", &recording.host_url),
            ("Director Link (Control Room)", &recording.director_url),
        ] {
            if let Some(url) = url {
                links.push(format!("{label}\n{url}\n"));
            }
        }
        sections.push(links.join("\n"));
    }

    sections.push(format!("## Show Notes\n\n{PLACEHOLDER}\n"));
    sections.push("## Files\n".to_owned());
    sections.push("## YouTube\n".to_owned());
    sections.join("\n")
}

fn episode_data(
    title: &str,
    iso: &str,
    base: &str,
    guest: Option<&str>,
    config: &ChannelConfig,
) -> String {
    let guest = guest.filter(|name| !name.trim().is_empty());
    let mut out = String::new();

    out.push_str(&format!("# Episode Data: {title}\n\n"));
    out.push_str(
        "This file tracks all IDs, URLs, and metadata for this episode across different production stages.\n\n",
    );

    out.push_str("## Episode Details\n\n");
    out.push_str(&format!("- **Episode Date**: {iso}\n"));
    out.push_str(&format!("- **Publication Date**: {PLACEHOLDER}\n"));
    out.push_str(&format!("- **Title**: {title}\n"));
    out.push_str(&format!(
        "- **Guest**: {}\n",
        guest.unwrap_or("None (solo episode)")
    ));
    out.push_str(&format!("- **Duration**: {PLACEHOLDER}\n\n"));

    out.push_str("## Guest Details\n\n");
    match guest {
        Some(name) => {
            out.push_str(&format!("- **Name**: {name}\n"));
            for label in [
                "Email",
                "Pronouns",
                "Organisation / Job Title",
                "LinkedIn",
                "Bluesky",
                "Website",
            ] {
                out.push_str(&format!("- **{label}**: {PLACEHOLDER}\n"));
            }
            out.push_str(&format!("\n### Bio\n\n{PLACEHOLDER}\n\n"));
        }
        None => out.push_str("(Solo episode - no guest)\n\n"),
    }

    out.push_str("## File Locations\n\n");
    out.push_str(&format!("- **Video File**: `{base}.mp4`\n"));
    out.push_str(&format!("- **Show Notes**: `{base}.md`\n"));
    out.push_str(&format!(
        "- **Description (formatted)**: `{base}.youtube-description.txt`\n\n"
    ));

    out.push_str("## YouTube\n\n### Video\n\n");
    out.push_str(&format!("- **Video ID**: {PLACEHOLDER}\n"));
    out.push_str(&format!("- **YouTube URL**: {PLACEHOLDER}\n"));
    out.push_str(&format!("- **YouTube Short URL**: {PLACEHOLDER}\n"));
    let privacy = config
        .youtube
        .as_ref()
        .and_then(|youtube| youtube.default_privacy.as_deref())
        .unwrap_or("unlisted");
    out.push_str(&format!("- **Privacy Status**: [{privacy}/public]\n"));
    out.push_str(&format!("- **Upload Date**: {PLACEHOLDER}\n"));
    if let Some(youtube) = &config.youtube {
        for (label, value) in [
            ("Category", &youtube.category),
            ("Language", &youtube.language),
            ("License", &youtube.license),
        ] {
            if let Some(value) = value {
                out.push_str(&format!("- **{label}**: {value}\n"));
            }
        }
    }

    out.push_str("\n### Tags Used\n\n```\n[To be generated]\n```\n\n");

    out.push_str("### Playlists to Add\n\n");
    let playlists = config.playlists.as_ref();
    let mut entries = Vec::new();
    if let Some(playlists) = playlists {
        if let Some(id) = &playlists.binge_watch_id {
            entries.push(format!("- [ ] Every episode ({id})\n"));
        }
        if let Some(id) = &playlists.audio_only_id {
            entries.push(format!(
                "- [ ] Audio-only version ({id}) - add manually in YouTube Studio\n"
            ));
        }
    }
    if entries.is_empty() {
        entries.push(format!("- [ ] {PLACEHOLDER}\n"));
    }
    out.push_str(&entries.concat());
    out.push_str("\nTopic-specific (relevant to this episode):\n\n- [ ] [TBD]\n\n");

    out.push_str("### Thumbnail\n\n");
    out.push_str("- **File**: [To be created]\n");
    out.push_str("- **Status**: Not yet created\n");
    out.push_str(&format!("- **Uploaded**: {PLACEHOLDER}\n\n"));

    out.push_str("## Guest Review\n\n");
    for label in [
        "Unlisted URL sent to guest",
        "Guest approval received",
        "Changed to public",
    ] {
        out.push_str(&format!("- **{label}**: {PLACEHOLDER}\n"));
    }

    out.push_str("\n## Social Media & Publicity\n\n");
    for platform in ["Bluesky", "Mastodon", "LinkedIn", "Instagram"] {
        out.push_str(&format!("### {platform}\n\n"));
        out.push_str(&format!("- **Post URL**: {PLACEHOLDER}\n"));
        out.push_str(&format!("- **Post Date**: {PLACEHOLDER}\n\n"));
        out.push_str(&format!("#### Draft\n\n```text\n{PLACEHOLDER}\n```\n\n"));
    }
    for (name, archive) in &config.archive {
        out.push_str(&format!("### {} Archive\n\n", title_case(name)));
        out.push_str(&format!(
            "- **Archive post URL**: {}\n",
            archive.topic_url.as_deref().unwrap_or(PLACEHOLDER)
        ));
        out.push_str(&format!("- **Posted**: {PLACEHOLDER}\n\n"));
    }

    out.push_str("## Manual YouTube Studio Steps (after upload)\n\n");
    out.push_str("- [ ] Verify the licence is as intended\n");
    out.push_str("- [ ] Check the content declaration\n");
    out.push_str("- [ ] Upload thumbnail when ready\n\n");

    out.push_str("## Notes\n\n[Recording context, topics covered, anything notable]\n");
    out
}

fn some_or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn title_case(value: &str) -> String {
    value
        .split(['-', '_'])
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Accept `YYYY-MM-DD` or `YYYY.MM.DD` and return the ISO form.
fn normalise_date(date: &str) -> Result<String> {
    let parts = date.split(['-', '.']).collect::<Vec<_>>();
    let valid = matches!(parts.as_slice(), [year, month, day]
        if year.len() == 4 && month.len() == 2 && day.len() == 2
            && parts.iter().all(|part| part.bytes().all(|b| b.is_ascii_digit())));
    if !valid {
        bail!("date must be YYYY-MM-DD, got '{date}'");
    }
    let (month, day) = (
        parts[1].parse::<u32>().unwrap_or(0),
        parts[2].parse::<u32>().unwrap_or(0),
    );
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        bail!("'{date}' is not a real calendar date");
    }
    Ok(format!("{}-{:02}-{:02}", parts[0], month, day))
}

/// Remove a date the user repeated inside the title, which would otherwise be
/// slugified into the folder name after the date prefix.
fn strip_leading_date(title: &str) -> &str {
    let trimmed = title.trim_start();
    let mut characters = trimmed.char_indices();
    let looks_like_date = (0..10).all(|index| match characters.next() {
        Some((_, character)) if matches!(index, 4 | 7) => matches!(character, '-' | '.' | '/'),
        Some((_, character)) => character.is_ascii_digit(),
        None => false,
    });
    if !looks_like_date {
        return trimmed;
    }
    trimmed[10..]
        .trim_start_matches([' ', '-', ':', '.', '\t'])
        .trim()
}

fn slugify(title: &str) -> String {
    let mut slug = String::new();
    for character in title.chars() {
        if character.is_ascii_alphanumeric() {
            slug.extend(character.to_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies_titles() {
        assert_eq!(
            slugify("Alan Hassey on rural informatics"),
            "alan-hassey-on-rural-informatics"
        );
        assert_eq!(
            slugify("SNOMED CT vs ICD: what's the difference?"),
            "snomed-ct-vs-icd-what-s-the-difference"
        );
        assert_eq!(slugify("  spaced  out  "), "spaced-out");
        assert_eq!(slugify("!!!"), "");
    }

    #[test]
    fn strips_a_date_repeated_in_the_title() {
        assert_eq!(strip_leading_date("2026-05-01 NHS numbers"), "NHS numbers");
        assert_eq!(
            strip_leading_date("2026.05.01 - NHS numbers"),
            "NHS numbers"
        );
        assert_eq!(strip_leading_date("NHS numbers"), "NHS numbers");
        assert_eq!(strip_leading_date("2026 in review"), "2026 in review");
    }

    #[test]
    fn normalises_and_rejects_dates() {
        assert_eq!(normalise_date("2026-07-29").unwrap(), "2026-07-29");
        assert_eq!(normalise_date("2026.07.29").unwrap(), "2026-07-29");
        for rejected in [
            "29-07-2026",
            "2026-7-9",
            "2026-13-01",
            "2026-07-32",
            "nonsense",
        ] {
            assert!(normalise_date(rejected).is_err(), "accepted {rejected}");
        }
    }

    #[test]
    fn titles_case_archive_names() {
        assert_eq!(title_case("baw_medical"), "Baw Medical");
        assert_eq!(title_case("forum"), "Forum");
    }
}
