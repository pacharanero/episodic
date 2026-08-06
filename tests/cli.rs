// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[test]
fn bare_invocation_is_helpful_and_successful() {
    let output = cargo_bin_cmd!("eps").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Local-first episode production automation"));
    assert!(stdout.contains("Commands:"));
}

#[test]
fn every_version_form_is_available() {
    for argument in ["--version", "-V", "-v", "--v", "-version"] {
        let output = cargo_bin_cmd!("eps").arg(argument).output().unwrap();
        assert!(output.status.success(), "version form failed: {argument}");
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .starts_with("eps ")
        );
    }
}

#[test]
fn version_command_emits_structured_json() {
    let output = cargo_bin_cmd!("eps")
        .args(["version", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["name"], "eps");
    assert!(
        value["version"]
            .as_str()
            .is_some_and(|version| !version.is_empty())
    );
}

#[test]
fn valid_channel_config_passes_and_invalid_config_fails() {
    let valid = fixture("channels/valid.toml");
    let output = cargo_bin_cmd!("eps")
        .args([
            "--config",
            valid.to_str().unwrap(),
            "--format",
            "json",
            "channel",
            "validate",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["valid"], true);

    let invalid = fixture("channels/invalid.toml");
    let output = cargo_bin_cmd!("eps")
        .args([
            "--config",
            invalid.to_str().unwrap(),
            "--format",
            "json",
            "channel",
            "validate",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["valid"], false);
    assert_eq!(report["issues"].as_array().unwrap().len(), 4);
}

#[test]
fn episode_inspection_reports_ready_fixture() {
    let fixture = EpisodeFixture::new(false);
    let output = cargo_bin_cmd!("eps")
        .args([
            "--format",
            "json",
            "episode",
            "inspect",
            fixture.episode.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["base"], EpisodeFixture::BASE);
    assert_eq!(report["stage"], "in-progress");
    assert_eq!(report["ready_for_upload"], true);
    assert_eq!(
        report["next_action"],
        "Plan an upload to the configured destination"
    );
}

#[test]
fn episode_validation_rejects_truncated_chapters() {
    let fixture = EpisodeFixture::new(true);
    let output = cargo_bin_cmd!("eps")
        .args([
            "--format",
            "json",
            "episode",
            "validate",
            fixture.episode.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let findings = report["findings"].as_array().unwrap();
    assert!(
        findings
            .iter()
            .any(|finding| finding["code"] == "chapter-coverage")
    );
}

#[test]
fn completions_generate_for_supported_shells() {
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let output = cargo_bin_cmd!("eps")
            .args(["completions", shell])
            .output()
            .unwrap();
        assert!(output.status.success(), "completion failed for {shell}");
        assert!(!output.stdout.is_empty());
    }
}

fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

struct EpisodeFixture {
    _temp: TempDir,
    episode: PathBuf,
}

impl EpisodeFixture {
    const BASE: &'static str = "2026.07.29-fixture-nhs-numbers";

    fn new(truncated_chapters: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let episode = temp.path().join("episodes/in-progress").join(Self::BASE);
        fs::create_dir_all(&episode).unwrap();
        let metadata =
            fs::read_to_string(fixture("episode-data/canonical.episode-data.md")).unwrap();
        fs::write(
            episode.join(format!("{}.episode-data.md", Self::BASE)),
            metadata,
        )
        .unwrap();
        fs::write(
            episode.join(format!("{}.processed.mp4", Self::BASE)),
            b"fixture video",
        )
        .unwrap();
        fs::write(
            episode.join(format!("{}.tags.txt", Self::BASE)),
            "nhs number,python,validation,checksum,digital health,clinical informatics,open source,testing,research,healthcare,everything digital health",
        )
        .unwrap();
        fs::write(
            episode.join(format!("{}.youtube-description.txt", Self::BASE)),
            "Fixture description.\n\nCHAPTERS\n\n0:00 Introduction\n1:30 End\n\nLINKS MENTIONED\n\nExample\nhttps://example.com\n\nMORE EVERYTHING DIGITAL HEALTH\n",
        )
        .unwrap();
        fs::write(
            episode.join(format!("{}.srt", Self::BASE)),
            "1\n00:00:00,000 --> 00:00:10,000\nIntroduction\n\n2\n00:01:30,000 --> 00:01:40,000\nConclusion\n",
        )
        .unwrap();
        let chapters = if truncated_chapters {
            "0:00 Introduction\n0:30 Middle\n"
        } else {
            "0:00 Introduction\n1:30 Conclusion\n"
        };
        fs::write(
            episode.join(format!("{}.chapters.txt", Self::BASE)),
            chapters,
        )
        .unwrap();
        fs::write(
            episode.join(format!("{}.thumbnail.png", Self::BASE)),
            png_header(1280, 720),
        )
        .unwrap();
        Self {
            _temp: temp,
            episode,
        }
    }
}

fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes
}
