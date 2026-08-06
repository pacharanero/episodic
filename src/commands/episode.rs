// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::document::{Change, Document};
use crate::episode;
use crate::output::{self, OutputFormat};
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
