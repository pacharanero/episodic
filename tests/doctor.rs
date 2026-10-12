// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

#![cfg(unix)]

use serde_json::Value;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

const IDENTITY: &str = "[channel]\nid = 'test'\nname = 'Test'\ntimezone = 'UTC'\n";
const METADATA: &str = "
[llm]
default_profile = 'main'
[llm.profiles.main]
provider = 'cli'
model = 'unverified-model'
command = 'fake-llm'
args = ['SECRET-ARGUMENT']
[llm.tasks]
custom-draft = 'main'
[metadata.social.other-draft]
max_characters = 300
[metadata.prompt_files]
examples = 'examples.txt'
[thumbnail]
font_path = 'font.ttf'
[youtube]
channel_id = 'test-channel'
[credentials.youtube]
client_secret_path = 'secret.json'
token_path = 'token.json'
";

fn file(root: &Path, name: &str, contents: &str, mode: u32) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, contents).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

fn workspace(extra: &str) -> TempDir {
    let temp = tempdir().unwrap();
    file(
        temp.path(),
        "eps.toml",
        &format!("{IDENTITY}{extra}"),
        0o600,
    );
    fs::create_dir(temp.path().join("episodes")).unwrap();
    for name in [
        "bin/ffmpeg",
        "bin/ffprobe",
        "bin/convert",
        "bin/fake-llm",
        "venv/bin/whisper",
    ] {
        // Any execution, including a version probe, must be observable.
        file(
            temp.path(),
            name,
            "#!/bin/sh\nprintf executed > executed\nexit 99\n",
            0o700,
        );
    }
    temp
}

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_eps"));
    command
        .env_clear()
        .env("PATH", root.join("bin"))
        .env("HOME", root)
        .current_dir(root);
    command.arg("doctor");
    command
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| panic!("{error}: {output:?}"))
}

fn check<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == name)
        .unwrap_or_else(|| panic!("missing {name}: {report}"))
}

fn provision(root: &Path) {
    for task in [
        "chapters",
        "tags",
        "thumbnail-quote",
        "youtube-description",
        "custom-draft",
        "other-draft",
    ] {
        file(root, &format!("prompts/{task}.md"), "{{transcript}}", 0o600);
    }
    for name in ["examples.txt", "font.ttf", "secret.json", "token.json"] {
        // Deliberately not valid credential JSON: doctor must not parse it.
        file(root, name, "DO-NOT-DISCLOSE-CREDENTIAL", 0o600);
    }
}

#[test]
fn reports_all_formats_without_running_tools_or_reading_credentials() {
    let temp = workspace(METADATA);
    let root = temp.path();
    provision(root);
    for format in ["text", "json", "yaml"] {
        let output = command(root)
            .args(["--format", format, "--dry-run"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout.clone()).unwrap();
        assert!(!text.contains("DO-NOT-DISCLOSE"));
        assert!(!text.contains("SECRET-ARGUMENT"));
        assert!(text.contains("version not checked"));
        assert!(text.contains("model availability unverified"));
        if format != "text" {
            let report: Value = if format == "json" {
                json(&output)
            } else {
                serde_yaml_ng::from_slice(&output.stdout).unwrap()
            };
            assert_eq!(report["ok"], true);
            assert_eq!(
                report["config_path"],
                fs::canonicalize(root.join("eps.toml"))
                    .unwrap()
                    .to_str()
                    .unwrap()
            );
            assert_eq!(
                check(&report, "whisper")["path"],
                root.join("venv/bin/whisper").to_str().unwrap()
            );
            assert_eq!(
                check(&report, "imagemagick (magick/convert)")["path"],
                root.join("bin/convert").to_str().unwrap()
            );
            for task in [
                "thumbnail-quote",
                "youtube-description",
                "custom-draft",
                "other-draft",
            ] {
                assert_eq!(
                    check(&report, &format!("prompt-template.{task}"))["status"],
                    "ok"
                );
            }
        }
        assert!(!root.join("executed").exists());
        assert_eq!(
            fs::read_to_string(root.join("token.json")).unwrap(),
            "DO-NOT-DISCLOSE-CREDENTIAL"
        );
    }
}

#[test]
fn accumulates_missing_prerequisites_before_failing() {
    let temp = workspace(METADATA);
    fs::remove_dir(temp.path().join("episodes")).unwrap();
    fs::remove_file(temp.path().join("bin/ffmpeg")).unwrap();
    let output = command(temp.path())
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report = json(&output);
    assert_eq!(report["ok"], false);
    for name in [
        "episodes",
        "ffmpeg",
        "thumbnail.font_path",
        "prompt-template.thumbnail-quote",
        "prompt-template.youtube-description",
        "prompt-file.examples",
        "credentials.youtube.token_path",
    ] {
        assert_eq!(check(&report, name)["status"], "error");
    }
    assert!(!temp.path().join("episodes").exists());
}

#[test]
fn incomplete_configured_features_fail_but_absent_optional_features_warn() {
    for (extra, name) in [
        (
            "[transcription]\nbackend = 'unsupported'\nmodel = 'test'\n",
            "transcription.backend",
        ),
        ("[thumbnail]\nwidth = 1280\n", "thumbnail.font_path"),
        ("[metadata]\nchapters_min = 6\n", "llm"),
        (
            "[youtube]\nchannel_id = 'test'\n[credentials.youtube]\nenv = 'SECRET_ENV'\n",
            "credentials.youtube.token_path",
        ),
    ] {
        let temp = workspace(extra);
        let output = command(temp.path())
            .env("SECRET_ENV", "DO-NOT-DISCLOSE")
            .args(["--format", "json"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(check(&json(&output), name)["status"], "error");
        assert!(!String::from_utf8_lossy(&output.stdout).contains("DO-NOT-DISCLOSE"));
    }
    let temp = workspace("");
    let output = command(temp.path())
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    for name in ["llm", "thumbnail.font_path", "youtube"] {
        assert_eq!(check(&json(&output), name)["status"], "warning");
    }
}

#[test]
fn valid_overrides_win_and_relative_assets_use_config_parent() {
    let temp = workspace(METADATA);
    provision(temp.path());
    let elsewhere = tempdir().unwrap();
    file(elsewhere.path(), "custom-tool", "must not execute", 0o700);
    let output = command(temp.path())
        .current_dir(elsewhere.path())
        .env("EPISODIC_FFMPEG_BIN", "./custom-tool")
        .args([
            "--config",
            temp.path().join("eps.toml").to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let report = json(&output);
    assert_eq!(check(&report, "ffmpeg")["path"], "./custom-tool");
    for (name, relative) in [
        ("episodes", "episodes"),
        ("thumbnail.font_path", "font.ttf"),
        ("prompt-file.examples", "examples.txt"),
        ("credentials.youtube.token_path", "token.json"),
    ] {
        assert_eq!(
            check(&report, name)["path"],
            temp.path().join(relative).to_str().unwrap()
        );
    }
}

#[test]
fn invalid_overrides_never_fall_back_to_available_tools() {
    let temp = workspace(METADATA);
    provision(temp.path());
    for (variable, name) in [
        ("EPISODIC_FFMPEG_BIN", "ffmpeg"),
        ("EPISODIC_MAGICK_BIN", "imagemagick (magick/convert)"),
        ("EPISODIC_CONVERT_BIN", "imagemagick (magick/convert)"),
        ("EPISODIC_WHISPER_BIN", "whisper"),
        ("EPISODIC_FAKE_LLM_BIN", "llm.profiles.main"),
    ] {
        for invalid in ["absent", "", "eps.toml", "episodes"] {
            let output = command(temp.path())
                .env(variable, invalid)
                .args(["--format", "json"])
                .output()
                .unwrap();
            assert!(!output.status.success(), "{variable}={invalid}: {output:?}");
            assert_eq!(check(&json(&output), name)["status"], "error");
        }
    }
}

#[test]
fn path_precedes_whisper_fallback_and_magick_precedes_convert() {
    let temp = workspace("");
    for name in ["whisper", "magick"] {
        file(temp.path(), &format!("bin/{name}"), "never execute", 0o700);
    }
    let output = command(temp.path())
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report = json(&output);
    assert_eq!(
        check(&report, "whisper")["path"],
        temp.path().join("bin/whisper").to_str().unwrap()
    );
    assert_eq!(
        check(&report, "imagemagick (magick/convert)")["path"],
        temp.path().join("bin/magick").to_str().unwrap()
    );
}

#[test]
fn credentials_check_modes_and_types_without_opening_files() {
    let temp = workspace(METADATA);
    provision(temp.path());
    for (mode, expected) in [(0o600, "ok"), (0o644, "warning"), (0o000, "error")] {
        fs::set_permissions(
            temp.path().join("token.json"),
            fs::Permissions::from_mode(mode),
        )
        .unwrap();
        let output = command(temp.path())
            .args(["--format", "json"])
            .output()
            .unwrap();
        assert_eq!(
            check(&json(&output), "credentials.youtube.token_path")["status"],
            expected
        );
        assert_eq!(output.status.success(), expected != "error");
    }
    fs::remove_file(temp.path().join("token.json")).unwrap();
    fs::create_dir(temp.path().join("token.json")).unwrap();
    let output = command(temp.path())
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert_eq!(
        check(&json(&output), "credentials.youtube.token_path")["status"],
        "error"
    );
}

#[test]
fn environment_credential_references_are_reported_without_being_read() {
    let temp = workspace(
        "[youtube]\nchannel_id = 'test'\n[credentials.youtube]\nclient_secret_env = 'YOUTUBE_CLIENT_SECRET_PATH'\ntoken_env = 'YOUTUBE_TOKEN_PATH'\n",
    );
    let output = command(temp.path())
        .env("YOUTUBE_CLIENT_SECRET_PATH", "DO-NOT-DISCLOSE")
        .env("YOUTUBE_TOKEN_PATH", "DO-NOT-DISCLOSE")
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report = json(&output);
    for name in [
        "credentials.youtube.client_secret_env",
        "credentials.youtube.token_env",
    ] {
        assert_eq!(check(&report, name)["status"], "warning");
    }
    assert!(!String::from_utf8_lossy(&output.stdout).contains("DO-NOT-DISCLOSE"));
}

#[test]
fn distinguishes_unused_providers_from_routed_unimplemented_providers() {
    for (routing, success, status) in [
        ("", true, "warning"),
        ("\n[llm.tasks]\nchapters = 'future'\n", false, "error"),
    ] {
        let temp = workspace(&format!(
            "[llm]\ndefault_profile = 'main'\n[llm.profiles.main]\nprovider = 'cli'\nmodel = 'test'\ncommand = 'fake-llm'\n[llm.profiles.future]\nprovider = 'native'\nmodel = 'test'\n{routing}"
        ));
        provision(temp.path());
        let output = command(temp.path())
            .args(["--format", "json"])
            .output()
            .unwrap();
        assert_eq!(output.status.success(), success, "{output:?}");
        assert_eq!(
            check(&json(&output), "llm.profiles.future")["status"],
            status
        );
    }
}

#[test]
fn selection_is_explicit_or_pwd_only_and_failures_remain_structured() {
    let temp = workspace("");
    let elsewhere = tempdir().unwrap();
    symlink(
        temp.path().join("eps.toml"),
        elsewhere.path().join("selected.toml"),
    )
    .unwrap();
    let output = command(temp.path())
        .current_dir(elsewhere.path())
        .args(["--config", "selected.toml", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        json(&output)["workspace_root"],
        fs::canonicalize(temp.path()).unwrap().to_str().unwrap()
    );
    fs::create_dir(temp.path().join("child")).unwrap();
    for selector in [None, Some("missing.toml")] {
        let mut cmd = command(temp.path());
        cmd.current_dir(temp.path().join("child"))
            .env("EPS_CONFIG", temp.path().join("eps.toml"))
            .args(["--format", "yaml"]);
        if let Some(selector) = selector {
            cmd.args(["--config", selector]);
        }
        let output = cmd.output().unwrap();
        assert!(!output.status.success());
        let report: Value = serde_yaml_ng::from_slice(&output.stdout).unwrap();
        assert_eq!(check(&report, "config")["status"], "error");
    }
    for invalid in [
        "not TOML SECRET-VALUE",
        "[channel]\nid = 'bad ID'\nname = 'Test'\ntimezone = 'UTC'",
        "[unknown]\nsecret = 'SECRET-VALUE'",
    ] {
        file(temp.path(), "eps.toml", invalid, 0o600);
        let output = command(temp.path())
            .args(["--format", "json"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(check(&json(&output), "config")["status"], "error");
        assert!(!String::from_utf8_lossy(&output.stdout).contains("SECRET-VALUE"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("SECRET-VALUE"));
    }
}
