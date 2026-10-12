// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Provider-neutral model access, prompt rendering, and bounded retries.
//!
//! A model failure must never produce a completed artefact. Every generated
//! output is validated against its task contract before it is allowed to
//! become a file, and a run that cannot produce a valid result fails loudly
//! rather than writing something plausible.

use crate::channel::{ChannelConfig, LlmProfile};
use crate::tool::Tool;
use anyhow::{Context, Result, bail};
use serde_json::json;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// How many times a task is attempted before giving up.
pub const MAX_ATTEMPTS: u32 = 3;

/// A model chosen for one task.
pub struct Model<'a> {
    pub profile_name: String,
    profile: &'a LlmProfile,
}

impl<'a> Model<'a> {
    /// Resolve the profile a task is routed to, falling back to the default.
    pub fn for_task(config: &'a ChannelConfig, task: &str) -> Result<Self> {
        let llm = config
            .llm
            .as_ref()
            .context("this channel has no [llm] configuration, so it cannot generate metadata")?;
        let profile_name = llm
            .tasks
            .get(task)
            .unwrap_or(&llm.default_profile)
            .to_owned();
        let profile = llm.profiles.get(&profile_name).with_context(|| {
            format!("llm profile '{profile_name}' is not defined in this channel's config")
        })?;
        Ok(Self {
            profile_name,
            profile,
        })
    }

    pub fn describe(&self) -> String {
        format!("{} ({})", self.profile_name, self.profile.model)
    }

    /// Send a prompt and return the response.
    pub fn complete(&self, prompt: &str) -> Result<String> {
        match self.profile.provider.as_str() {
            "cli" => self.complete_via_cli(prompt),
            "anthropic" => self.complete_via_anthropic(prompt),
            other => bail!(
                "llm profile '{}' uses provider '{other}', which is not implemented yet; only provider = \"cli\" works today",
                self.profile_name
            ),
        }
    }

    fn complete_via_anthropic(&self, prompt: &str) -> Result<String> {
        let variable = self.profile.api_key_env.as_deref().with_context(|| {
            format!(
                "llm profile '{}' uses provider 'anthropic' and must set api_key_env",
                self.profile_name
            )
        })?;
        let api_key = std::env::var(variable).with_context(|| {
            format!(
                "environment variable '{variable}' is required for Anthropic profile '{}'",
                self.profile_name
            )
        })?;
        if api_key.is_empty() {
            bail!(
                "environment variable '{variable}' is empty for Anthropic profile '{}'",
                self.profile_name
            );
        }

        let timeout = Duration::from_secs(self.profile.timeout_seconds.unwrap_or(180));
        let response = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()
            .context("could not create the Anthropic HTTP client")?
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": self.profile.model,
                "max_tokens": 4096,
                "messages": [{"role": "user", "content": prompt}],
            }))
            .send()
            .context("Anthropic Messages request failed")?;
        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .context("Anthropic returned an unreadable response")?;
        if !status.is_success() {
            let message = body["error"]["message"]
                .as_str()
                .unwrap_or("no error message returned");
            bail!("Anthropic Messages request failed with {status}: {message}");
        }
        anthropic_text(&body).context("Anthropic returned a response without text content")
    }

    fn complete_via_cli(&self, prompt: &str) -> Result<String> {
        let command = self.profile.command.as_deref().with_context(|| {
            format!(
                "llm profile '{}' uses provider 'cli' and must set command",
                self.profile_name
            )
        })?;
        let tool = Tool::find(command, &[])?;
        let arguments = self
            .profile
            .args
            .iter()
            .map(|argument| argument.replace("{model}", &self.profile.model))
            .collect::<Vec<_>>();
        let timeout = Duration::from_secs(self.profile.timeout_seconds.unwrap_or(180));
        let by_argument = self.profile.input.as_deref() == Some("argument");

        let mut arguments = arguments;
        if by_argument {
            arguments.push(prompt.to_owned());
        }

        let mut child = Command::new(&tool.path)
            .args(&arguments)
            .stdin(if by_argument {
                Stdio::null()
            } else {
                Stdio::piped()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("could not run {}", tool.path.display()))?;

        // Feed stdin from its own thread: a prompt carrying a condensed
        // transcript can exceed the pipe buffer, and writing it inline would
        // deadlock against a child that has not started reading yet.
        if !by_argument {
            let mut stdin = child.stdin.take().expect("stdin was piped");
            let prompt = prompt.to_owned();
            thread::spawn(move || {
                let _ = stdin.write_all(prompt.as_bytes());
            });
        }

        let pid = child.id();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(child.wait_with_output());
        });

        let output = match receiver.recv_timeout(timeout) {
            Ok(result) => result.context("could not collect model output")?,
            Err(_) => {
                terminate(pid);
                bail!(
                    "{} timed out after {}s for profile '{}'",
                    command,
                    timeout.as_secs(),
                    self.profile_name
                );
            }
        };

        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() {
            bail!(
                "{command} failed for profile '{}'{}",
                self.profile_name,
                if stderr.trim().is_empty() {
                    String::new()
                } else {
                    format!(":\n{}", stderr.trim())
                }
            );
        }

        let response = String::from_utf8_lossy(&output.stdout).into_owned();
        if response.trim().is_empty() {
            bail!(
                "{command} returned empty output for profile '{}'; an empty response is a failure, not an artefact",
                self.profile_name
            );
        }
        Ok(response)
    }
}

fn anthropic_text(response: &serde_json::Value) -> Option<String> {
    response["content"].as_array().and_then(|blocks| {
        blocks
            .iter()
            .filter(|block| block["type"] == "text")
            .filter_map(|block| block["text"].as_str())
            .map(str::to_owned)
            .reduce(|mut text, next| {
                text.push_str(&next);
                text
            })
    })
}

#[cfg(unix)]
fn terminate(pid: u32) {
    // Safety: sending a signal to a pid we spawned and have not reaped.
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn terminate(_pid: u32) {}

/// Render a `{{placeholder}}` template from the prompt library.
///
/// Every placeholder must be supplied. An unresolved one would be sent to the
/// model as literal braces, which produces confidently wrong output rather
/// than an error.
pub fn render(template: &str, values: &[(String, String)]) -> Result<String> {
    let mut rendered = String::with_capacity(template.len());
    let mut rest = template;
    let mut missing = Vec::new();

    while let Some(start) = rest.find("{{") {
        rendered.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            rendered.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let name = &after[..end];
        // Last match wins, so a caller's --var overrides configuration.
        match values.iter().rev().find(|(key, _)| key == name) {
            Some((_, value)) => rendered.push_str(value),
            None => {
                missing.push(name.to_owned());
                rendered.push_str(&format!("{{{{{name}}}}}"));
            }
        }
        rest = &after[end + 2..];
    }
    rendered.push_str(rest);

    if !missing.is_empty() {
        missing.sort();
        missing.dedup();
        bail!("prompt variables not supplied: {}", missing.join(", "));
    }
    Ok(rendered)
}

/// Load a named template from the workspace prompt library.
pub fn load_template(workspace_root: &Path, name: &str) -> Result<String> {
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        bail!("prompt name '{name}' must use lowercase letters, digits, and hyphens");
    }
    let path = workspace_root.join("prompts").join(format!("{name}.md"));
    std::fs::read_to_string(&path)
        .with_context(|| format!("prompt template not found: {}", path.display()))
}

/// Flatten channel configuration into dotted template variables, so a prompt
/// can refer to `channel.voice` or `description.contact_email`.
pub fn config_values(config_path: &Path) -> Result<Vec<(String, String)>> {
    let source = std::fs::read_to_string(config_path)?;
    let value: toml::Value = toml::from_str(&source)?;
    let mut values = Vec::new();
    flatten(&value, "", &mut values);
    Ok(values)
}

fn flatten(value: &toml::Value, prefix: &str, out: &mut Vec<(String, String)>) {
    match value {
        toml::Value::Table(table) => {
            for (key, child) in table {
                let name = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(child, &name, out);
            }
        }
        toml::Value::Array(items) => {
            let joined = items
                .iter()
                .map(|item| match item {
                    toml::Value::String(text) => text.clone(),
                    other => other.to_string(),
                })
                .collect::<Vec<_>>()
                .join(" ");
            out.push((prefix.to_owned(), joined));
        }
        toml::Value::String(text) => out.push((prefix.to_owned(), text.clone())),
        other => out.push((prefix.to_owned(), other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_supplied_variables() {
        let values = vec![
            ("transcript".to_owned(), "words".to_owned()),
            ("channel.voice".to_owned(), "direct".to_owned()),
        ];
        assert_eq!(
            render("Say {{transcript}} in a {{channel.voice}} voice.", &values).unwrap(),
            "Say words in a direct voice."
        );
    }

    #[test]
    fn refuses_to_send_an_unresolved_placeholder_to_a_model() {
        let error = render("Use {{transcript}} and {{missing}}", &[])
            .unwrap_err()
            .to_string();
        assert!(error.contains("missing"));
        assert!(error.contains("transcript"));
    }

    #[test]
    fn a_later_value_overrides_an_earlier_one() {
        let values = vec![
            ("title".to_owned(), "from config".to_owned()),
            ("title".to_owned(), "from the command line".to_owned()),
        ];
        assert_eq!(
            render("{{title}}", &values).unwrap(),
            "from the command line"
        );
    }

    #[test]
    fn leaves_text_without_placeholders_alone() {
        assert_eq!(render("plain prompt", &[]).unwrap(), "plain prompt");
        assert_eq!(
            render("an { unmatched brace", &[]).unwrap(),
            "an { unmatched brace"
        );
    }

    #[test]
    fn extracts_all_text_blocks_from_an_anthropic_response() {
        let response = json!({
            "content": [
                {"type": "thinking", "thinking": "internal"},
                {"type": "text", "text": "first "},
                {"type": "text", "text": "second"}
            ]
        });
        assert_eq!(anthropic_text(&response).as_deref(), Some("first second"));
    }

    #[test]
    fn flattens_config_into_dotted_names() {
        let value: toml::Value = toml::from_str(
            "[channel]\nvoice = 'direct'\n[hashtags]\ndefault = ['#a', '#b']\n[thumbnail]\nwidth = 1280\n",
        )
        .unwrap();
        let mut values = Vec::new();
        flatten(&value, "", &mut values);
        let lookup = |name: &str| {
            values
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        };
        assert_eq!(lookup("channel.voice").as_deref(), Some("direct"));
        assert_eq!(lookup("hashtags.default").as_deref(), Some("#a #b"));
        assert_eq!(lookup("thumbnail.width").as_deref(), Some("1280"));
    }

    #[test]
    fn rejects_prompt_names_that_could_escape_the_library() {
        let root = Path::new("/workspace");
        assert!(load_template(root, "../../etc/passwd").is_err());
        assert!(load_template(root, "Chapters").is_err());
    }
}
