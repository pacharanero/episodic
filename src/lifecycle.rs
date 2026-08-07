// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Lifecycle stages, episode discovery, and guarded stage promotion.
//!
//! An episode's stage is expressed by which directory holds its folder, so
//! promotion is a directory move. There is no status field to disagree with
//! the filesystem.

use crate::episode;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Stages in production order. `not-for-publishing` is a terminal branch
/// reachable from any stage rather than a step in the sequence.
pub const STAGES: &[&str] = &[
    "in-planning",
    "in-progress",
    "edited",
    "published",
    "not-for-publishing",
];

const ABANDONED: &str = "not-for-publishing";
const PUBLISHED: &str = "published";

#[derive(Debug, Serialize)]
pub struct EpisodeEntry {
    pub stage: String,
    pub base: String,
    pub date: Option<String>,
    pub slug: Option<String>,
    pub path: PathBuf,
    /// True when the episode has its canonical metadata file.
    pub has_metadata: bool,
}

#[derive(Debug, Serialize)]
pub struct EpisodeList {
    pub episodes_path: PathBuf,
    pub episodes: Vec<EpisodeEntry>,
}

impl EpisodeList {
    pub fn as_text(&self) -> String {
        if self.episodes.is_empty() {
            return format!("No episodes found in {}", self.episodes_path.display());
        }
        let mut lines = Vec::new();
        for stage in STAGES {
            let staged = self
                .episodes
                .iter()
                .filter(|episode| episode.stage == *stage)
                .collect::<Vec<_>>();
            if staged.is_empty() {
                continue;
            }
            lines.push(format!("{stage} ({})", staged.len()));
            lines.extend(staged.iter().map(|episode| {
                let flag = if episode.has_metadata {
                    ""
                } else {
                    "  (no metadata)"
                };
                format!("  {}{flag}", episode.base)
            }));
        }
        lines.join("\n")
    }
}

/// Every episode folder under the workspace, newest first within each stage.
///
/// Folders are recognised by the `YYYY.MM.DD-slug` naming convention, so loose
/// documents and working directories sitting beside episodes are ignored. Back
/// catalogue episodes filed under a year subdirectory are included; other
/// nested directories, such as frame dumps or editor project folders, are not
/// traversed.
pub fn list(episodes_path: &Path, stage_filter: Option<&str>) -> Result<EpisodeList> {
    let mut episodes = Vec::new();
    for stage in STAGES {
        if stage_filter.is_some_and(|wanted| wanted != *stage) {
            continue;
        }
        let directory = episodes_path.join(stage);
        if !directory.is_dir() {
            continue;
        }

        let mut roots = vec![directory.clone()];
        roots.extend(
            directories(&directory)?
                .into_iter()
                .filter(|path| is_year(path)),
        );

        let mut staged = Vec::new();
        for root in roots {
            for path in directories(&root)? {
                let Some(base) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                let Some((date, slug)) = split_base(base) else {
                    continue;
                };
                staged.push(EpisodeEntry {
                    stage: (*stage).to_owned(),
                    base: base.to_owned(),
                    date: Some(date),
                    slug: Some(slug),
                    has_metadata: path.join(format!("{base}.episode-data.md")).is_file(),
                    path,
                });
            }
        }
        staged.sort_by(|left, right| right.base.cmp(&left.base));
        episodes.extend(staged);
    }

    Ok(EpisodeList {
        episodes_path: episodes_path.to_path_buf(),
        episodes,
    })
}

#[derive(Debug, Serialize)]
pub struct Promotion {
    pub base: String,
    pub from_stage: String,
    pub to_stage: String,
    pub from: PathBuf,
    pub to: PathBuf,
    pub applied: bool,
    /// Guard conditions that were overridden by `--force`.
    pub overridden: Vec<String>,
}

impl Promotion {
    pub fn as_text(&self) -> String {
        let heading = if self.applied {
            "Promoted"
        } else {
            "Would promote"
        };
        let mut lines = vec![format!(
            "{heading} {} from {} to {}",
            self.base, self.from_stage, self.to_stage
        )];
        lines.push(format!(
            "  {} -> {}",
            self.from.display(),
            self.to.display()
        ));
        lines.extend(
            self.overridden
                .iter()
                .map(|reason| format!("  overridden: {reason}")),
        );
        lines.join("\n")
    }
}

/// Plan a stage change, refusing the moves that would misrepresent reality.
///
/// Guards are refusals rather than warnings because the stage directory is the
/// record: an episode filed under `published` that was never uploaded is
/// indistinguishable from one that was.
pub fn plan_promotion(input: &Path, to_stage: &str, force: bool) -> Result<Promotion> {
    if !STAGES.contains(&to_stage) {
        bail!(
            "unknown stage '{to_stage}'; expected one of {}",
            STAGES.join(", ")
        );
    }

    let report = episode::inspect(input)?;
    let from_stage = report.stage.clone();
    if from_stage == "unknown" {
        bail!(
            "{} is not inside a lifecycle directory, so it has no stage to promote from",
            report.episode_path.display()
        );
    }
    if from_stage == to_stage {
        bail!("{} is already in {to_stage}", report.base);
    }

    let mut blocked = Vec::new();
    if to_stage == PUBLISHED && report.fields.video_id.is_none() {
        blocked.push(
            "no Video ID recorded, so this episode has not been shown to exist on the destination"
                .to_owned(),
        );
    }
    if to_stage != ABANDONED
        && from_stage != ABANDONED
        && let (Some(from), Some(to)) = (position(&from_stage), position(to_stage))
        && to < from
    {
        blocked.push(format!(
            "{to_stage} is earlier in the lifecycle than {from_stage}"
        ));
    }
    if from_stage == ABANDONED {
        blocked.push(format!("{ABANDONED} is a deliberate end state"));
    }

    if !blocked.is_empty() && !force {
        bail!(
            "cannot promote {} to {to_stage}: {}; pass --force to override",
            report.base,
            blocked.join("; ")
        );
    }

    let destination = report
        .episode_path
        .parent()
        .context("episode folder has no parent")?
        .parent()
        .context("lifecycle directory has no parent")?
        .join(to_stage);
    let to = destination.join(&report.base);
    if to.exists() {
        bail!(
            "{} already exists; the same episode must not appear in two stages",
            to.display()
        );
    }

    Ok(Promotion {
        base: report.base,
        from_stage,
        to_stage: to_stage.to_owned(),
        from: report.episode_path,
        to,
        applied: false,
        overridden: blocked,
    })
}

pub fn apply_promotion(promotion: &mut Promotion) -> Result<()> {
    let destination = promotion
        .to
        .parent()
        .context("promotion target has no parent")?;
    fs::create_dir_all(destination)
        .with_context(|| format!("could not create {}", destination.display()))?;
    fs::rename(&promotion.from, &promotion.to).with_context(|| {
        format!(
            "could not move {} to {}",
            promotion.from.display(),
            promotion.to.display()
        )
    })?;
    promotion.applied = true;
    Ok(())
}

fn position(stage: &str) -> Option<usize> {
    STAGES.iter().position(|known| *known == stage)
}

fn directories(path: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = fs::read_dir(path)
        .with_context(|| format!("could not read {}", path.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn is_year(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.len() == 4 && name.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Split `YYYY.MM.DD-slug` into an ISO date and the slug.
pub fn split_base(base: &str) -> Option<(String, String)> {
    let (date, slug) = base.split_at(base.char_indices().nth(10)?.0);
    let slug = slug.strip_prefix('-')?;
    let parts = date.split('.').collect::<Vec<_>>();
    match parts.as_slice() {
        [year, month, day]
            if year.len() == 4
                && month.len() == 2
                && day.len() == 2
                && date.bytes().all(|b| b.is_ascii_digit() || b == b'.') =>
        {
            (!slug.is_empty()).then(|| (format!("{year}-{month}-{day}"), slug.to_owned()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_episode_folder_names() {
        assert_eq!(
            split_base("2026.07.29-nhs-numbers"),
            Some(("2026-07-29".to_owned(), "nhs-numbers".to_owned()))
        );
        for rejected in [
            "2026.07.29",
            "2026.07.29-",
            "not-an-episode",
            "2026-07-29-nhs-numbers",
            "202.07.29-slug",
        ] {
            assert_eq!(split_base(rejected), None, "accepted {rejected}");
        }
    }

    #[test]
    fn orders_stages_for_promotion_guards() {
        assert!(position("in-planning") < position("published"));
        assert_eq!(position("nonsense"), None);
    }
}
