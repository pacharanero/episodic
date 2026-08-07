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
fn setting_fields_changes_only_the_managed_lines() {
    let fixture = EpisodeFixture::new(false);
    let before = fixture.metadata();

    let output = cargo_bin_cmd!("eps")
        .args([
            "--format",
            "json",
            "episode",
            "set",
            fixture.episode.to_str().unwrap(),
            "Video ID=dQw4w9WgXcQ",
            "YouTube URL=https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["applied"], true);
    assert_eq!(report["changes"].as_array().unwrap().len(), 2);

    let after = fixture.metadata();
    let differing = before
        .lines()
        .zip(after.lines())
        .filter(|(before, after)| before != after)
        .count();
    assert_eq!(before.lines().count(), after.lines().count());
    assert_eq!(differing, 2);
    assert!(after.contains("- **Video ID**: dQw4w9WgXcQ"));
    assert!(after.contains("## Custom Human Context"));
    assert!(after.contains("This paragraph and heading are not managed fields"));
    assert!(after.contains("Original human-reviewed Bluesky draft."));
}

#[test]
fn a_dry_run_reports_the_plan_and_writes_nothing() {
    let fixture = EpisodeFixture::new(false);
    let before = fixture.metadata();

    let output = cargo_bin_cmd!("eps")
        .args([
            "--dry-run",
            "episode",
            "set",
            fixture.episode.to_str().unwrap(),
            "Video ID=dQw4w9WgXcQ",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Would update"));
    assert!(stdout.contains("[To be added] -> dQw4w9WgXcQ"));
    assert_eq!(fixture.metadata(), before);
}

#[test]
fn an_existing_value_is_protected_until_forced() {
    let fixture = EpisodeFixture::new(false);
    let before = fixture.metadata();

    let output = cargo_bin_cmd!("eps")
        .args([
            "episode",
            "set",
            fixture.episode.to_str().unwrap(),
            "Title=Replaced Without Asking",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("--force")
    );
    assert_eq!(fixture.metadata(), before);

    let output = cargo_bin_cmd!("eps")
        .args([
            "episode",
            "set",
            fixture.episode.to_str().unwrap(),
            "--force",
            "Title=Replaced Deliberately",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        fixture
            .metadata()
            .contains("- **Title**: Replaced Deliberately")
    );
}

#[test]
fn repeating_an_update_is_a_no_op() {
    let fixture = EpisodeFixture::new(false);
    let arguments = [
        "--format",
        "json",
        "episode",
        "set",
        fixture.episode.to_str().unwrap(),
        "Video ID=dQw4w9WgXcQ",
    ];
    assert!(
        cargo_bin_cmd!("eps")
            .args(arguments)
            .output()
            .unwrap()
            .status
            .success()
    );
    let after_first = fixture.metadata();

    let output = cargo_bin_cmd!("eps").args(arguments).output().unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["applied"], false);
    assert!(report["changes"].as_array().unwrap().is_empty());
    assert_eq!(fixture.metadata(), after_first);
}

#[test]
fn replacing_a_draft_preserves_the_rest_of_the_document() {
    let fixture = EpisodeFixture::new(false);
    let draft = fixture.episode.join("draft.txt");
    fs::write(&draft, "A new Bluesky draft.\nWith two lines.\n").unwrap();

    let output = cargo_bin_cmd!("eps")
        .args([
            "episode",
            "set-draft",
            fixture.episode.to_str().unwrap(),
            "Bluesky",
            "--from-file",
            draft.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let after = fixture.metadata();
    assert!(after.contains("```text\nA new Bluesky draft.\nWith two lines.\n```"));
    assert!(!after.contains("Original human-reviewed Bluesky draft."));
    assert!(after.contains("- **Post URL**: [To be added]"));
    assert!(after.contains("## Custom Human Context"));
    assert!(after.ends_with("no live remote IDs or media.\n"));
}

#[test]
fn publication_updates_touch_only_their_own_lines() {
    let fixture = EpisodeFixture::with_document("publication", false);
    let before = fixture.metadata();
    let episode = fixture.episode.to_str().unwrap().to_owned();

    let run = |arguments: &[&str]| {
        let output = cargo_bin_cmd!("eps").args(arguments).output().unwrap();
        assert!(
            output.status.success(),
            "{arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };

    run(&[
        "episode",
        "mark-playlist",
        &episode,
        "PLfixturesafety",
        "--note",
        "Added 2026-08-07T09:00:00Z",
    ]);
    run(&["episode", "add-platform", &episode, "Mastodon"]);

    let tags = fixture.episode.join("tags.txt");
    fs::write(&tags, "nhs, digital health, informatics\n").unwrap();
    run(&[
        "episode",
        "set-tags",
        &episode,
        "--from-file",
        tags.to_str().unwrap(),
    ]);

    let after = fixture.metadata();
    assert!(after.contains("- [x] Clinical Safety (PLfixturesafety) - Added 2026-08-07T09:00:00Z"));
    assert!(after.contains("```\nnhs, digital health, informatics\n```"));
    assert!(after.contains("**Tag character count**: 32/500 ✓"));

    // The manual studio steps use the same checkbox syntax and must be untouched.
    for untouched in [
        "- [ ] Verify license is \"Creative Commons - Attribution\"",
        "- [ ] More options > Remixing > \"Don't allow remixing\"",
        "- [ ] Content declaration > \"No\" to altered content",
        "- [ ] Audio-Only Podcast Version (PLfixtureaudio) - add manually in YouTube Studio",
        "- [x] EVERYTHING Fixture Channel - BINGE WATCH (PLfixturebinge) - Added 2026-07-29T10:00:00Z",
        "Original human-reviewed Bluesky draft.",
        "Human prose that must survive every update exactly.",
    ] {
        assert!(after.contains(untouched), "lost: {untouched}");
    }

    let mastodon = after.find("### Mastodon").unwrap();
    assert!(mastodon > after.find("### Bluesky").unwrap());
    assert!(mastodon < after.find("## Manual YouTube Studio Steps").unwrap());
    assert_eq!(before.lines().count() + 11, after.lines().count());
}

#[test]
fn marking_an_unlisted_playlist_fails_without_changing_anything() {
    let fixture = EpisodeFixture::with_document("publication", false);
    let before = fixture.metadata();
    let output = cargo_bin_cmd!("eps")
        .args([
            "episode",
            "mark-playlist",
            fixture.episode.to_str().unwrap(),
            "PLnotinthisdocument",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("PLnotinthisdocument")
    );
    assert_eq!(fixture.metadata(), before);
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
        Self::with_document("canonical", truncated_chapters)
    }

    fn with_document(shape: &str, truncated_chapters: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let episode = temp.path().join("episodes/in-progress").join(Self::BASE);
        fs::create_dir_all(&episode).unwrap();
        let metadata =
            fs::read_to_string(fixture(&format!("episode-data/{shape}.episode-data.md"))).unwrap();
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

    fn metadata(&self) -> String {
        fs::read_to_string(self.episode.join(format!("{}.episode-data.md", Self::BASE))).unwrap()
    }
}

fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes
}
