// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Local prerequisite inspection only: never open credentials or spawn tools.

use crate::channel;
use crate::output::{self, OutputFormat};
use crate::tool::Tool;
use anyhow::{Result, bail};
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Ok,
    Warning,
    Error,
}

#[derive(Serialize)]
struct Check {
    name: String,
    status: Status,
    path: Option<PathBuf>,
    message: String,
}

#[derive(Default, Serialize)]
struct Report {
    config_path: Option<PathBuf>,
    workspace_root: Option<PathBuf>,
    ok: bool,
    checks: Vec<Check>,
}

impl Report {
    fn add(&mut self, name: &str, status: Status, path: Option<PathBuf>, message: &str) {
        self.checks.push(Check {
            name: name.to_owned(),
            status,
            path,
            message: message.to_owned(),
        });
    }

    fn file(&mut self, name: &str, path: PathBuf, credential: bool) {
        let (status, message) = match fs::metadata(&path) {
            Ok(meta) if meta.is_file() => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = meta.permissions().mode();
                    if mode & 0o444 == 0 {
                        (
                            Status::Error,
                            "No read permission bits; grant the running user read access",
                        )
                    } else if credential && mode & 0o077 != 0 {
                        (
                            Status::Warning,
                            "Credential accessible to group/others; restrict to owner-only permissions (chmod 600)",
                        )
                    } else {
                        (
                            Status::Ok,
                            "Regular file present; read permission bits set; contents not inspected",
                        )
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = credential;
                    (
                        Status::Warning,
                        "Regular file present; permissions not checked on this platform",
                    )
                }
            }
            Ok(_) => (
                Status::Error,
                "Expected a regular file; correct the configured path",
            ),
            Err(_) => (
                Status::Error,
                "File missing or inaccessible; provision it or correct the configured path",
            ),
        };
        self.add(name, status, Some(path), message);
    }

    fn tool(&mut self, name: &str, result: Result<Tool>, failure: Status) {
        match result {
            Ok(tool) => self.add(
                name,
                Status::Ok,
                Some(tool.path),
                "Executable file found; version not checked (no subprocess execution)",
            ),
            Err(error) => self.add(
                name,
                failure,
                None,
                &format!(
                    "{error}; install the executable or correct its override; version not checked"
                ),
            ),
        }
    }
}

pub fn run(explicit: Option<&Path>, format: OutputFormat) -> Result<()> {
    let mut report = Report::default();
    match channel::resolve_path(explicit) {
        Ok(path) => {
            report.workspace_root = path.parent().map(Path::to_path_buf);
            report.config_path = Some(path.clone());
            match channel::load(&path) {
                Ok(config) => {
                    report.add("config", Status::Ok, Some(path.clone()), "Selected configuration parsed and validated");
                    if inspect(&mut report, &config, &path).is_err() {
                        report.add("workspace", Status::Error, None, "Workspace paths became inaccessible during inspection; check the selected config and retry");
                    }
                }
                // TOML diagnostics can include source lines, including misplaced secrets.
                Err(_) => report.add("config", Status::Error, Some(path), "Configuration unreadable, malformed, or invalid; check schema and required fields locally (source omitted for safety)"),
            }
        }
        Err(_) => {
            report.config_path =
                std::path::absolute(explicit.unwrap_or(Path::new("eps.toml"))).ok();
            report.add("config", Status::Error, report.config_path.clone(), "Cannot resolve selected configuration; run in a workspace containing eps.toml or pass --config; no fallback attempted");
        }
    }
    report.add("inspection-limits", Status::Warning, None, "Versions not checked; model availability unverified; no tools executed, models downloaded, or remote calls made. File contents, credential validity, effective access/ACLs, and parent-directory permissions not checked");
    report.ok = !report
        .checks
        .iter()
        .any(|check| check.status == Status::Error);
    let mut lines = vec![format!(
        "Doctor: {}",
        if report.ok {
            "no prerequisite errors"
        } else {
            "prerequisite errors found"
        }
    )];
    if let Some(root) = &report.workspace_root {
        lines.push(format!("Workspace: {}", root.display()));
    }
    for check in &report.checks {
        let status = match check.status {
            Status::Ok => "ok",
            Status::Warning => "warning",
            Status::Error => "error",
        };
        let path = check
            .path
            .as_ref()
            .map(|path| format!(" [{}]", path.display()))
            .unwrap_or_default();
        lines.push(format!("{status}: {}{path}: {}", check.name, check.message));
    }
    output::write(format, &report, &lines.join("\n"))?;
    if !report.ok {
        bail!("doctor found missing or invalid prerequisites");
    }
    Ok(())
}

fn inspect(report: &mut Report, config: &channel::ChannelConfig, config_path: &Path) -> Result<()> {
    let root = channel::workspace_root(config_path)?;
    let episodes = config.episodes_path(config_path)?;
    report.add(
        "episodes",
        if episodes.is_dir() {
            Status::Ok
        } else {
            Status::Error
        },
        Some(episodes),
        "Episode directory must exist and be accessible; create it or correct paths.episodes",
    );
    for name in ["ffmpeg", "ffprobe"] {
        report.tool(name, Tool::find(name, &[]), Status::Error);
    }
    report.tool(
        "imagemagick (magick/convert)",
        crate::thumbnail::imagemagick(),
        Status::Error,
    );
    report.tool(
        "whisper",
        Tool::find("whisper", &[root.join("venv/bin/whisper")]),
        Status::Error,
    );
    if let Some(policy) = &config.transcription
        && policy.backend != "whisper"
    {
        report.add("transcription.backend", Status::Error, None, "Configured backend is not implemented; production currently invokes Whisper regardless; configure backend = 'whisper'");
    }
    report.add(
        "transcription.model",
        Status::Warning,
        None,
        "Whisper model availability unverified; no cache inspection or download attempted",
    );
    match config
        .thumbnail
        .as_ref()
        .and_then(|policy| policy.font_path.as_ref())
    {
        Some(path) => report.file("thumbnail.font_path", root.join(path), false),
        None => report.add(
            "thumbnail.font_path",
            if config.thumbnail.is_some() {
                Status::Error
            } else {
                Status::Warning
            },
            None,
            "Set thumbnail.font_path before composing thumbnails",
        ),
    }

    let mut tasks = BTreeSet::new();
    if config.llm.is_some() || config.metadata.is_some() {
        tasks.extend(["chapters", "tags", "thumbnail-quote", "youtube-description"]);
    }
    if let Some(metadata) = &config.metadata {
        tasks.extend(metadata.social.keys().map(String::as_str));
        for (name, path) in &metadata.prompt_files {
            report.file(&format!("prompt-file.{name}"), root.join(path), false);
        }
    }
    if let Some(llm) = &config.llm {
        tasks.extend(llm.tasks.keys().map(String::as_str));
        let used: BTreeSet<_> = tasks
            .iter()
            .map(|task| llm.tasks.get(*task).unwrap_or(&llm.default_profile))
            .collect();
        for (name, profile) in &llm.profiles {
            let severity = if used.contains(name) {
                Status::Error
            } else {
                Status::Warning
            };
            match profile.provider.as_str() {
                "cli" => report.tool(
                    &format!("llm.profiles.{name}"),
                    Tool::find(profile.command.as_deref().unwrap_or(""), &[]),
                    severity,
                ),
                "anthropic" => report.add(
                    &format!("llm.profiles.{name}"),
                    Status::Ok,
                    None,
                    "Anthropic Messages API; api_key_env is resolved only when generating metadata",
                ),
                _ => report.add(&format!("llm.profiles.{name}"), severity, None, "Provider not implemented; use provider = 'cli' for routed tasks (unused profiles are warnings)"),
            }
        }
        for task in &tasks {
            let profile = llm.tasks.get(*task).unwrap_or(&llm.default_profile);
            report.add(
                &format!("task.{task}"),
                Status::Ok,
                None,
                &format!("Routes to profile '{profile}'; model availability unverified"),
            );
        }
    } else {
        report.add(
            "llm",
            if config.metadata.is_some() {
                Status::Error
            } else {
                Status::Warning
            },
            None,
            "No LLM configured; set [llm] before generating metadata",
        );
    }
    for task in tasks {
        if task.is_empty()
            || !task
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            report.add(
                "prompt-template",
                Status::Error,
                None,
                "Task names must use lowercase letters, digits, and hyphens",
            );
        } else {
            report.file(
                &format!("prompt-template.{task}"),
                root.join("prompts").join(format!("{task}.md")),
                false,
            );
        }
    }

    if config.youtube.is_some() || config.credentials.contains_key("youtube") {
        for (name, path, environment) in [
            (
                "client_secret",
                config
                    .credentials
                    .get("youtube")
                    .and_then(|reference| reference.client_secret_path.as_ref()),
                config
                    .credentials
                    .get("youtube")
                    .and_then(|reference| reference.client_secret_env.as_deref()),
            ),
            (
                "token",
                config
                    .credentials
                    .get("youtube")
                    .and_then(|reference| reference.token_path.as_ref()),
                config
                    .credentials
                    .get("youtube")
                    .and_then(|reference| reference.token_env.as_deref()),
            ),
        ] {
            match (path, environment) {
                (Some(path), None) => report.file(
                    &format!("credentials.youtube.{name}_path"),
                    root.join(path),
                    true,
                ),
                (None, Some(environment)) => report.add(
                    &format!("credentials.youtube.{name}_env"),
                    Status::Warning,
                    None,
                    &format!("Environment reference {environment} configured; its value and target file are not checked"),
                ),
                (None, None) => report.add(
                    &format!("credentials.youtube.{name}_path"),
                    Status::Error,
                    None,
                    "Set a file path or environment reference; the native YouTube adapter requires both credential files",
                ),
                (Some(_), Some(_)) => report.add(
                    &format!("credentials.youtube.{name}"),
                    Status::Error,
                    None,
                    "Set only one credential reference; channel validation should reject this configuration",
                ),
            }
        }
    } else {
        report.add(
            "youtube",
            Status::Warning,
            None,
            "Optional YouTube integration not configured; credentials not inspected",
        );
    }
    Ok(())
}
