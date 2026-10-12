// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::output::{self, OutputFormat};
use anyhow::{Context, Result};
use clap::Command;
use clap_complete::{Shell, generate, generate_to};
use serde::Serialize;
use std::env;
use std::io;
use std::path::{Path, PathBuf};

pub fn generate_completion(
    shell: Shell,
    directory: Option<&Path>,
    dry_run: bool,
    mut command: Command,
) -> Result<()> {
    if let Some(directory) = directory {
        if dry_run {
            eprintln!(
                "Would write {shell} completion for eps to {}",
                directory.display()
            );
            return Ok(());
        }
        std::fs::create_dir_all(directory).with_context(|| {
            format!(
                "could not create completion directory {}",
                directory.display()
            )
        })?;
        let path = generate_to(shell, &mut command, "eps", directory)
            .context("could not generate completion file")?;
        println!("{}", path.display());
    } else {
        generate(shell, &mut command, "eps", &mut io::stdout());
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct InstallReport {
    shell: String,
    path: PathBuf,
    applied: bool,
    setup_hint: Option<String>,
}

impl InstallReport {
    fn as_text(&self) -> String {
        let action = if self.applied {
            "Installed"
        } else {
            "Would install"
        };
        let mut lines = vec![format!(
            "{action} {} completion: {}",
            self.shell,
            self.path.display()
        )];
        if let Some(hint) = &self.setup_hint {
            lines.push(format!("Setup: {hint}"));
        }
        lines.join("\n")
    }
}

pub fn install(
    shell: Option<Shell>,
    directory: Option<&Path>,
    dry_run: bool,
    mut command: Command,
    format: OutputFormat,
) -> Result<()> {
    let shell = shell.or_else(detect_shell).context(
        "could not detect a supported shell; rerun with --shell bash, zsh, fish, powershell, or elvish",
    )?;
    let directory = match directory {
        Some(directory) => directory.to_path_buf(),
        None => default_directory(shell)
            .context("could not determine a completion directory; rerun with --directory <path>")?,
    };
    let report = InstallReport {
        shell: shell.to_string(),
        path: directory.join(completion_filename(shell)),
        applied: !dry_run,
        setup_hint: setup_hint(shell),
    };
    if !dry_run {
        std::fs::create_dir_all(&directory).with_context(|| {
            format!(
                "could not create completion directory {}",
                directory.display()
            )
        })?;
        generate_to(shell, &mut command, "eps", &directory)
            .context("could not generate completion file")?;
    }
    output::write(format, &report, &report.as_text())
}

fn detect_shell() -> Option<Shell> {
    if env::var_os("PSModulePath").is_some() {
        return Some(Shell::PowerShell);
    }
    let shell = env::var_os("SHELL")?;
    match Path::new(&shell).file_name()?.to_str()? {
        "bash" => Some(Shell::Bash),
        "zsh" => Some(Shell::Zsh),
        "fish" => Some(Shell::Fish),
        "elvish" => Some(Shell::Elvish),
        _ => None,
    }
}

fn default_directory(shell: Shell) -> Option<PathBuf> {
    let home = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"))?;
    let home = PathBuf::from(home);
    match shell {
        Shell::Bash => Some(
            env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/share"))
                .join("bash-completion/completions"),
        ),
        Shell::Zsh => Some(home.join(".zfunc")),
        Shell::Fish => Some(
            env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"))
                .join("fish/completions"),
        ),
        Shell::PowerShell => Some(home.join("Documents/PowerShell/Completions")),
        Shell::Elvish => Some(
            env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"))
                .join("elvish/lib"),
        ),
        _ => None,
    }
}

fn completion_filename(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => "eps",
        Shell::Zsh => "_eps",
        Shell::Fish => "eps.fish",
        Shell::PowerShell => "eps.ps1",
        Shell::Elvish => "eps.elv",
        _ => "eps",
    }
}

fn setup_hint(shell: Shell) -> Option<String> {
    match shell {
        Shell::Zsh => Some("add `fpath=(~/.zfunc $fpath)` before `compinit` in your zsh setup if it is not already present".to_owned()),
        Shell::PowerShell => Some("dot-source this file from your PowerShell profile if your installation does not load completion scripts from this directory".to_owned()),
        _ => None,
    }
}
