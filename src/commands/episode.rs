// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::channel;
use crate::document::{Change, Document};
use crate::episode;
use crate::lifecycle;
use crate::output::{self, OutputFormat};
use crate::skeleton;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub fn inspect(path: &Path, format: OutputFormat) -> Result<()> {
    let report = episode::inspect(path)?;
    output::write(format, &report, &report.as_text())
}

pub fn validate(path: &Path, format: OutputFormat) -> Result<()> {
    let report = episode::inspect(path)?;
    let valid = report.is_valid();
    output::write(format, &report, &report.as_text())?;
    if !valid {
        bail!("episode validation failed");
    }
    Ok(())
}

/// Create a new episode folder in `in-planning`.
pub fn new(
    config_path: &Path,
    date: &str,
    title: &str,
    guest: Option<&str>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let config = channel::load(config_path)?;
    let episodes_path = config.episodes_path(config_path)?;
    let mut plan = skeleton::plan(&episodes_path, date, title, guest, &config)?;
    if !dry_run {
        skeleton::create(&mut plan)?;
    }
    output::write(format, &plan, &plan.as_text())
}

/// List episodes by lifecycle stage.
pub fn list(config_path: &Path, stage: Option<&str>, format: OutputFormat) -> Result<()> {
    let config = channel::load(config_path)?;
    let episodes_path = config.episodes_path(config_path)?;
    let list = lifecycle::list(&episodes_path, stage)?;
    output::write(format, &list, &list.as_text())
}

/// Move an episode to another lifecycle stage.
pub fn promote(
    path: &Path,
    stage: &str,
    force: bool,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let mut promotion = lifecycle::plan_promotion(path, stage, force)?;
    if !dry_run {
        lifecycle::apply_promotion(&mut promotion)?;
    }
    output::write(format, &promotion, &promotion.as_text())
}

/// The result of a document update, reported identically for a dry run and an
/// apply so the plan a user reviews is the plan that gets written.
#[derive(Debug, Serialize)]
struct UpdateReport {
    metadata_path: PathBuf,
    applied: bool,
    changes: Vec<Change>,
}

impl UpdateReport {
    fn as_text(&self) -> String {
        if self.changes.is_empty() {
            return format!("No changes needed in {}", self.metadata_path.display());
        }
        let heading = if self.applied {
            "Updated"
        } else {
            "Would update"
        };
        let mut lines = vec![format!(
            "{heading} {} ({} change{})",
            self.metadata_path.display(),
            self.changes.len(),
            if self.changes.len() == 1 { "" } else { "s" }
        )];
        lines.extend(self.changes.iter().map(Change::as_text));
        lines.join("\n")
    }
}

/// Set managed `- **Label**: value` fields, preserving everything else.
pub fn set(
    path: &Path,
    assignments: &[String],
    section: Option<&str>,
    force: bool,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let (_, metadata_path, _) = episode::locate(path)?;
    let mut document = Document::read(&metadata_path)?;

    let mut changes = Vec::new();
    for assignment in assignments {
        let (label, value) = assignment.split_once('=').with_context(|| {
            format!("expected an assignment of the form 'Label=value', got '{assignment}'")
        })?;
        if let Some(change) = document.set_field(label.trim(), section, value.trim(), force)? {
            changes.push(change);
        }
    }

    finish(document, metadata_path, changes, dry_run, format)
}

/// Replace a fenced draft block within a social platform section.
pub fn set_draft(
    path: &Path,
    platform: &str,
    heading: &str,
    text: &str,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let (_, metadata_path, _) = episode::locate(path)?;
    let mut document = Document::read(&metadata_path)?;
    let changes = document
        .set_draft(platform, heading, text)?
        .into_iter()
        .collect::<Vec<_>>();

    finish(document, metadata_path, changes, dry_run, format)
}

/// Replace the generated tag list.
pub fn set_tags(path: &Path, tags: &str, dry_run: bool, format: OutputFormat) -> Result<()> {
    let (_, metadata_path, _) = episode::locate(path)?;
    let mut document = Document::read(&metadata_path)?;
    let changes = document.set_tags(tags)?;

    finish(document, metadata_path, changes, dry_run, format)
}

/// Record that one playlist was successfully added.
pub fn mark_playlist(
    path: &Path,
    playlist: &str,
    note: Option<&str>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let (_, metadata_path, _) = episode::locate(path)?;
    let mut document = Document::read(&metadata_path)?;
    let changes = document
        .mark_playlist(playlist, note)?
        .into_iter()
        .collect::<Vec<_>>();

    finish(document, metadata_path, changes, dry_run, format)
}

/// Add a missing social platform section.
pub fn add_platform(
    path: &Path,
    platform: &str,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let (_, metadata_path, _) = episode::locate(path)?;
    let mut document = Document::read(&metadata_path)?;
    let changes = document
        .add_platform(platform)?
        .into_iter()
        .collect::<Vec<_>>();

    finish(document, metadata_path, changes, dry_run, format)
}

/// Write only when something actually changed, so a no-op update never
/// rewrites the file or updates its modification time.
fn finish(
    document: Document,
    metadata_path: PathBuf,
    changes: Vec<Change>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let applied = !dry_run && !changes.is_empty();
    if applied {
        document.write()?;
    }
    let report = UpdateReport {
        metadata_path,
        applied,
        changes,
    };
    output::write(format, &report, &report.as_text())
}
