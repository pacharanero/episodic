// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Generated metadata: chapters, tags, descriptions, quotes, and social drafts.
//!
//! Generation, validation, and retry live together because they are one
//! operation. A model failure must never leave a completed artefact, so
//! nothing is written until an attempt has passed its task contract.

use crate::channel::{self, MetadataPolicy};
use crate::llm::{self, MAX_ATTEMPTS, Model};
use crate::output::{self, OutputFormat};
use crate::transcript;
use crate::validate;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Approximate seconds of speech per condensed transcript block.
const BLOCK_SECONDS: f64 = 30.0;

#[derive(Debug, Serialize)]
pub struct Generated {
    pub task: String,
    pub output_path: Option<PathBuf>,
    pub model: String,
    pub attempts: u32,
    pub applied: bool,
    pub skipped: bool,
    /// Contract failures from attempts that were rejected, kept so a run that
    /// eventually succeeded still shows what the model got wrong.
    pub rejected: Vec<String>,
    pub content: String,
}

impl Generated {
    pub fn as_text(&self) -> String {
        if self.skipped {
            return format!(
                "Already generated: {}\n{}",
                display_path(self.output_path.as_deref()),
                self.content.trim_end()
            );
        }
        let heading = match (&self.output_path, self.applied) {
            (Some(path), true) => format!("Wrote {}", path.display()),
            (Some(path), false) => format!("Would write {}", path.display()),
            (None, _) => format!("Generated {}", self.task),
        };
        let mut lines = vec![format!(
            "{heading} using {} (attempt {} of {MAX_ATTEMPTS})",
            self.model, self.attempts
        )];
        lines.extend(
            self.rejected
                .iter()
                .map(|reason| format!("  rejected an attempt: {reason}")),
        );
        lines.push("---".to_owned());
        lines.push(self.content.trim_end().to_owned());
        lines.join("\n")
    }
}

fn display_path(path: Option<&Path>) -> String {
    path.map_or_else(
        || "standard output".to_owned(),
        |path| path.display().to_string(),
    )
}

pub struct Request<'a> {
    pub task: &'a str,
    pub config_path: &'a Path,
    /// Transcript to condense into the prompt, when the task needs one.
    pub transcript_path: Option<&'a Path>,
    /// Extra template variables, as `name=value`.
    pub variables: &'a [String],
    pub output: Option<&'a Path>,
    pub force: bool,
    pub dry_run: bool,
}

/// Generate one artefact, retrying until it satisfies its contract.
pub fn generate(request: &Request, format: OutputFormat) -> Result<()> {
    let config = channel::load(request.config_path)?;
    let policy = config.metadata.unwrap_or_default();
    let workspace = channel::workspace_root(request.config_path)?;

    if let Some(output) = request.output
        && !request.force
        && output.metadata().is_ok_and(|meta| meta.len() > 0)
    {
        let content = fs::read_to_string(output)?;
        return report(
            Generated {
                task: request.task.to_owned(),
                output_path: Some(output.to_path_buf()),
                model: String::new(),
                attempts: 0,
                applied: false,
                skipped: true,
                rejected: Vec::new(),
                content,
            },
            format,
        );
    }

    let mut values = llm::config_values(request.config_path)?;
    let mut transcript_end = None;
    if let Some(path) = request.transcript_path {
        let source = fs::read_to_string(path)
            .with_context(|| format!("could not read transcript {}", path.display()))?;
        let (digest, end) = transcript::digest(&source, BLOCK_SECONDS)?;
        eprintln!(
            "Transcript: {} blocks, ending at {}.",
            digest.lines().count(),
            transcript::format_timestamp(end as f64)
        );
        values.push(("transcript".to_owned(), digest));
        transcript_end = Some(end);
    }
    for variable in request.variables {
        let (name, value) = variable
            .split_once('=')
            .with_context(|| format!("expected NAME=VALUE, got '{variable}'"))?;
        values.push((name.trim().to_owned(), value.to_owned()));
    }

    let template = llm::load_template(&workspace, request.task)?;
    let prompt = llm::render(&template, &values)?;

    let config = channel::load(request.config_path)?;
    let model = Model::for_task(&config, request.task)?;

    if request.dry_run {
        return report(
            Generated {
                task: request.task.to_owned(),
                output_path: request.output.map(Path::to_path_buf),
                model: model.describe(),
                attempts: 0,
                applied: false,
                skipped: false,
                rejected: Vec::new(),
                content: prompt,
            },
            format,
        );
    }

    let (content, attempts, rejected) =
        attempt_until_valid(&model, &prompt, request.task, &policy, transcript_end)?;

    if let Some(output) = request.output {
        write_atomically(output, &content)?;
    }

    report(
        Generated {
            task: request.task.to_owned(),
            output_path: request.output.map(Path::to_path_buf),
            model: model.describe(),
            attempts,
            applied: request.output.is_some(),
            skipped: false,
            rejected,
            content,
        },
        format,
    )
}

/// Ask the model until an answer passes, or give up.
///
/// Retrying is worth it because these failures are usually transient shape
/// errors, but the attempt count is bounded: a model that cannot satisfy the
/// contract will not be talked into it, and silently writing the last bad
/// answer would be worse than failing.
fn attempt_until_valid(
    model: &Model,
    prompt: &str,
    task: &str,
    policy: &MetadataPolicy,
    transcript_end: Option<u64>,
) -> Result<(String, u32, Vec<String>)> {
    let mut rejected = Vec::new();
    for attempt in 1..=MAX_ATTEMPTS {
        eprintln!(
            "Generating {task} with {} ({attempt}/{MAX_ATTEMPTS})...",
            model.describe()
        );
        let response = match model.complete(prompt) {
            Ok(response) => response,
            Err(error) if attempt < MAX_ATTEMPTS => {
                rejected.push(format!("{error:#}"));
                continue;
            }
            Err(error) => return Err(error),
        };
        let content = response.trim().to_owned();
        match validate::check(task, &content, policy, transcript_end) {
            Ok(()) => return Ok((content, attempt, rejected)),
            Err(error) => rejected.push(format!("{error:#}")),
        }
    }
    bail!(
        "{task} did not satisfy its contract after {MAX_ATTEMPTS} attempts:\n  {}",
        rejected.join("\n  ")
    )
}

fn write_atomically(output: &Path, content: &str) -> Result<()> {
    let directory = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    fs::create_dir_all(&directory)?;
    let mut temporary = tempfile::Builder::new()
        .prefix(&format!(
            ".{}.",
            output.file_name().unwrap_or_default().to_string_lossy()
        ))
        .tempfile_in(&directory)?;
    use std::io::Write;
    writeln!(temporary, "{}", content.trim_end())?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(output)
        .with_context(|| format!("could not write {}", output.display()))?;
    Ok(())
}

fn report(generated: Generated, format: OutputFormat) -> Result<()> {
    let text = generated.as_text();
    output::write(format, &generated, &text)
}

/// Validate an existing artefact without generating anything.
pub fn check(
    config_path: &Path,
    task: &str,
    input: &Path,
    transcript_path: Option<&Path>,
    format: OutputFormat,
) -> Result<()> {
    let config = channel::load(config_path)?;
    let policy = config.metadata.unwrap_or_default();
    let content =
        fs::read_to_string(input).with_context(|| format!("could not read {}", input.display()))?;
    let transcript_end = transcript_path
        .map(|path| -> Result<u64> {
            let source = fs::read_to_string(path)?;
            Ok(transcript::digest(&source, BLOCK_SECONDS)?.1)
        })
        .transpose()?;

    #[derive(Serialize)]
    struct Report<'a> {
        task: &'a str,
        input: &'a Path,
        valid: bool,
        issue: Option<String>,
    }

    let result = validate::check(task, content.trim(), &policy, transcript_end);
    let report = Report {
        task,
        input,
        valid: result.is_ok(),
        issue: result.as_ref().err().map(|error| format!("{error:#}")),
    };
    let text = match &report.issue {
        None => format!("{} satisfies the {task} contract", input.display()),
        Some(issue) => format!(
            "{} does not satisfy the {task} contract: {issue}",
            input.display()
        ),
    };
    output::write(format, &report, &text)?;
    if result.is_err() {
        bail!("{task} validation failed");
    }
    Ok(())
}
