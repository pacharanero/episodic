// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::episode;
use crate::output::{self, OutputFormat};
use anyhow::{Result, bail};
use std::path::Path;

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
