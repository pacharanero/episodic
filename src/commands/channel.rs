// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::channel;
use crate::output::{self, OutputFormat};
use anyhow::{Result, bail};
use std::path::Path;

pub fn list(path: &Path, format: OutputFormat) -> Result<()> {
    let entries = channel::list(path)?;
    let text = entries
        .iter()
        .map(|entry| {
            let status = if entry.valid { "valid" } else { "invalid" };
            format!(
                "{}\t{}\t{status}",
                entry.id.as_deref().unwrap_or("?"),
                entry.name.as_deref().unwrap_or("?")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    output::write(format, &entries, &text)
}

pub fn show(path: &Path, format: OutputFormat) -> Result<()> {
    let config = channel::load(path)?;
    let summary = config.summary(path);
    let text = format!(
        "Channel: {} ({})\nConfig: {}\nHost: {}\nTimezone: {}\nSections: {}",
        summary.name,
        summary.id,
        summary.config_path.display(),
        display_or_dash(&summary.host),
        display_or_dash(&summary.timezone),
        summary.sections.join(", ")
    );
    output::write(format, &summary, &text)
}

pub fn validate(path: &Path, format: OutputFormat) -> Result<()> {
    let report = channel::validate(path);
    let text = if report.valid {
        format!("Valid channel config: {}", path.display())
    } else {
        format!(
            "Invalid channel config: {}\n{}",
            path.display(),
            report
                .issues
                .iter()
                .map(|issue| format!("- {issue}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    output::write(format, &report, &text)?;
    if !report.valid {
        bail!("channel configuration is invalid");
    }
    Ok(())
}

fn display_or_dash(value: &str) -> &str {
    if value.is_empty() { "-" } else { value }
}
