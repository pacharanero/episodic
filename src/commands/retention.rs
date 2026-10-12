// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::channel;
use crate::output::{self, OutputFormat};
use crate::retention;
use anyhow::{Context, Result, bail};
use std::path::Path;

pub fn audit(config_path: &Path, stages: &[String], format: OutputFormat) -> Result<()> {
    let config = channel::load(config_path)?;
    let retention_days = config
        .storage
        .as_ref()
        .and_then(|storage| storage.raw_media_retention_days)
        .context("storage.raw_media_retention_days is required for retention audit")?;
    let episodes_path = config.episodes_path(config_path)?;
    let report = retention::audit(&episodes_path, stages, retention_days)?;
    output::write(format, &report, &report.as_text())
}

pub fn cleanup(
    episode: &Path,
    include_review: bool,
    apply: bool,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    if apply && dry_run {
        bail!("--dry-run cannot be combined with --apply");
    }
    let plan = retention::cleanup_plan(episode, include_review)?;
    let report = if apply {
        retention::apply_cleanup(plan)?
    } else {
        plan
    };
    output::write(format, &report, &report.as_text())
}
