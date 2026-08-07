// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::channel::{self, AudioPolicy, TranscriptionPolicy};
use crate::media::{self, Media};
use crate::output::{self, OutputFormat};
use crate::thumbnail;
use anyhow::Result;
use std::path::Path;

fn audio_policy(config_path: &Path) -> Result<AudioPolicy> {
    Ok(channel::load(config_path)?.audio.unwrap_or_default())
}

pub fn extract(
    config_path: &Path,
    input: &Path,
    out: Option<&Path>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let outcome = Media::new()?.extract_audio(input, out, &audio_policy(config_path)?, dry_run)?;
    output::write(format, &outcome, &outcome.as_text())
}

pub fn process(
    config_path: &Path,
    input: &Path,
    out: Option<&Path>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let outcome = Media::new()?.process_audio(input, out, &audio_policy(config_path)?, dry_run)?;
    output::write(format, &outcome, &outcome.as_text())
}

pub fn analyse(config_path: &Path, input: &Path, format: OutputFormat) -> Result<()> {
    let report = Media::new()?.analyse_audio(input, &audio_policy(config_path)?)?;
    output::write(format, &report, &report.as_text())
}

pub fn transcribe(
    config_path: &Path,
    input: &Path,
    output_dir: Option<&Path>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let config = channel::load(config_path)?;
    let policy = config.transcription.unwrap_or_else(default_transcription);
    let workspace = channel::workspace_root(config_path)?;
    let outcome = media::transcribe(
        input,
        output_dir,
        &policy.model,
        policy.language.as_deref().unwrap_or("en"),
        policy.device.as_deref().unwrap_or("cpu"),
        &workspace,
        dry_run,
    )?;
    output::write(format, &outcome, &outcome.as_text())
}

fn default_transcription() -> TranscriptionPolicy {
    TranscriptionPolicy {
        backend: "whisper".to_owned(),
        model: "turbo".to_owned(),
        language: Some("en".to_owned()),
        device: Some("cpu".to_owned()),
    }
}

pub fn frames(input: &Path, count: u32, dry_run: bool, format: OutputFormat) -> Result<()> {
    let outcome = thumbnail::frames(input, count, dry_run)?;
    output::write(format, &outcome, &outcome.as_text())
}

pub fn compose(
    config_path: &Path,
    image: &Path,
    text: &str,
    out: Option<&Path>,
    no_quotes: bool,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let config = channel::load(config_path)?;
    let policy = config.thumbnail.unwrap_or_default();
    let quote = !no_quotes && policy.quote_by_default.unwrap_or(true);
    let workspace = channel::workspace_root(config_path)?;
    let outcome = thumbnail::compose(image, text, out, &policy, &workspace, quote, dry_run)?;
    output::write(format, &outcome, &outcome.as_text())
}
