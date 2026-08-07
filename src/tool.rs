// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Locating and running the external tools `eps` orchestrates.
//!
//! `episodic` does not reimplement media processing or transcription. It runs
//! the proven tools and is responsible for choosing their arguments, checking
//! what they produced, and reporting exactly what was run.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// An external program `eps` depends on.
#[derive(Clone, Debug, Serialize)]
pub struct Tool {
    pub name: String,
    pub path: PathBuf,
}

impl Tool {
    /// Resolve a tool, preferring an explicit override.
    ///
    /// The `EPISODIC_<NAME>_BIN` override exists so tests can substitute a
    /// fake, and so an operator can point at a build that is not on `PATH`.
    /// `fallbacks` are workspace-local paths tried before giving up, which is
    /// how the Whisper virtualenv is found.
    pub fn find(name: &str, fallbacks: &[PathBuf]) -> Result<Self> {
        let variable = format!("EPISODIC_{}_BIN", name.to_uppercase().replace('-', "_"));
        if let Ok(override_path) = std::env::var(&variable) {
            let path = PathBuf::from(&override_path);
            if !is_runnable(&path) {
                bail!("{variable} is set to '{override_path}', which is not an executable file");
            }
            return Ok(Self {
                name: name.to_owned(),
                path,
            });
        }

        if let Some(path) = which(name) {
            return Ok(Self {
                name: name.to_owned(),
                path,
            });
        }
        for candidate in fallbacks {
            if is_runnable(candidate) {
                return Ok(Self {
                    name: name.to_owned(),
                    path: candidate.clone(),
                });
            }
        }
        bail!("{name} not found on PATH; set {variable} to use a specific build")
    }

    /// The command line this tool would run, for dry-run output and logs.
    pub fn command_line<S: AsRef<OsStr>>(&self, arguments: &[S]) -> String {
        let mut line = vec![self.path.display().to_string()];
        line.extend(
            arguments
                .iter()
                .map(|argument| quote(&argument.as_ref().to_string_lossy())),
        );
        line.join(" ")
    }

    /// Run to completion, capturing output. A non-zero exit is an error and
    /// carries the tool's own diagnostics, which are usually the useful part.
    pub fn run<S: AsRef<OsStr>>(&self, arguments: &[S]) -> Result<Output> {
        let output = Command::new(&self.path)
            .args(arguments)
            .stdin(Stdio::null())
            .output()
            .with_context(|| format!("could not run {}", self.path.display()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr.trim();
            bail!(
                "{} failed ({}){}",
                self.name,
                describe(&output),
                if detail.is_empty() {
                    String::new()
                } else {
                    format!(":\n{detail}")
                }
            );
        }
        Ok(output)
    }

    /// Run, letting the tool draw on the terminal.
    ///
    /// Transcription takes minutes and prints its own progress, which is worth
    /// more to a waiting human than a silent process.
    pub fn run_attached<S: AsRef<OsStr>>(&self, arguments: &[S]) -> Result<()> {
        let status = Command::new(&self.path)
            .args(arguments)
            .stdin(Stdio::null())
            .status()
            .with_context(|| format!("could not run {}", self.path.display()))?;
        if !status.success() {
            bail!("{} failed ({status})", self.name);
        }
        Ok(())
    }
}

fn describe(output: &Output) -> String {
    match output.status.code() {
        Some(code) => format!("exit status {code}"),
        None => "terminated by signal".to_owned(),
    }
}

fn which(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|directory| directory.join(name))
            .find(|candidate| is_runnable(candidate))
    })
}

#[cfg(unix)]
fn is_runnable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.is_file()
        && std::fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_runnable(path: &Path) -> bool {
    path.is_file()
}

/// Quote an argument so a reported command line can be pasted into a shell.
fn quote(argument: &str) -> String {
    if !argument.is_empty()
        && argument
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_./=:,@+".contains(&byte))
    {
        return argument.to_owned();
    }
    format!("'{}'", argument.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_only_arguments_that_need_it() {
        assert_eq!(quote("-i"), "-i");
        assert_eq!(quote("/path/to/file.mp4"), "/path/to/file.mp4");
        assert_eq!(
            quote("highpass=f=80,afftdn=nf=-25"),
            "highpass=f=80,afftdn=nf=-25"
        );
        assert_eq!(quote("loudnorm=I=-16:TP=-1.5"), "loudnorm=I=-16:TP=-1.5");
        assert_eq!(quote("a|b"), "'a|b'");
        assert_eq!(quote("$(rm -rf /)"), "'$(rm -rf /)'");
        assert_eq!(quote("with space"), "'with space'");
        assert_eq!(quote(""), "''");
        assert_eq!(quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn reports_a_pasteable_command_line() {
        let tool = Tool {
            name: "ffmpeg".to_owned(),
            path: PathBuf::from("/usr/bin/ffmpeg"),
        };
        assert_eq!(
            tool.command_line(&["-i", "in.mp4", "-af", "a=1,b=2", "out file.mp4"]),
            "/usr/bin/ffmpeg -i in.mp4 -af a=1,b=2 'out file.mp4'"
        );
    }
}
