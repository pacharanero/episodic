// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::Result;
use clap::ValueEnum;
use serde::Serialize;
use std::io::{self, Write};

/// Output encoding used by every command.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text.
    #[default]
    Text,
    /// Stable machine-readable JSON.
    Json,
    /// Human-readable structured YAML.
    Yaml,
}

pub fn write<T: Serialize + ?Sized>(format: OutputFormat, value: &T, text: &str) -> Result<()> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    match format {
        OutputFormat::Text => writeln!(output, "{text}")?,
        OutputFormat::Json => serde_json::to_writer_pretty(&mut output, value)?,
        OutputFormat::Yaml => serde_yaml_ng::to_writer(&mut output, value)?,
    }
    if format != OutputFormat::Text {
        writeln!(output)?;
    }
    Ok(())
}
