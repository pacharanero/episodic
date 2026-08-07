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
fn a_new_episode_is_built_from_channel_configuration() {
    let workspace = Workspace::new();
    let output = workspace
        .eps(&[
            "--format",
            "json",
            "episode",
            "new",
            "2026-09-15",
            "Alan Hassey on rural informatics",
            "--guest",
            "Dr Alan Hassey",
        ])
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["applied"], true);
    assert_eq!(
        report["base"],
        "2026.09.15-alan-hassey-on-rural-informatics"
    );

    let episode = workspace
        .root
        .join("episodes/in-planning/2026.09.15-alan-hassey-on-rural-informatics");
    let data = fs::read_to_string(
        episode.join("2026.09.15-alan-hassey-on-rural-informatics.episode-data.md"),
    )
    .unwrap();

    // Every channel-specific value comes from the config, not from the code.
    assert!(data.contains("- [ ] Every episode (PLfixturebinge)"));
    assert!(data.contains("- **Language**: en-GB"));
    assert!(data.contains("- **Guest**: Dr Alan Hassey"));
    assert!(data.contains("https://forum.invalid/archive"));

    let notes =
        fs::read_to_string(episode.join("2026.09.15-alan-hassey-on-rural-informatics.md")).unwrap();
    assert!(notes.contains("https://example.invalid/guest"));
    assert!(notes.contains("Fixture Host"));

    // The skeleton is immediately usable by the document commands.
    for arguments in [
        vec![
            "episode",
            "mark-playlist",
            episode.to_str().unwrap(),
            "PLfixturebinge",
        ],
        vec![
            "episode",
            "set",
            episode.to_str().unwrap(),
            "Video ID=abc123",
        ],
    ] {
        workspace.eps(&arguments).unwrap();
    }
}

#[test]
fn creating_the_same_episode_twice_fails_without_touching_it() {
    let workspace = Workspace::new();
    workspace
        .eps(&["episode", "new", "2026-09-15", "Rural informatics"])
        .unwrap();
    let episode = workspace
        .root
        .join("episodes/in-planning/2026.09.15-rural-informatics");
    let before = fs::read_to_string(episode.join("2026.09.15-rural-informatics.md")).unwrap();

    let output = workspace.run(&["episode", "new", "2026-09-15", "Rural informatics"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("already exists")
    );
    assert_eq!(
        fs::read_to_string(episode.join("2026.09.15-rural-informatics.md")).unwrap(),
        before
    );
}

#[test]
fn a_failed_new_episode_leaves_no_folder_behind() {
    let workspace = Workspace::new();
    let output = workspace.run(&["episode", "new", "2026-13-45", "Impossible date"]);
    assert!(!output.status.success());
    assert!(!workspace.root.join("episodes/in-planning").exists());
}

#[test]
fn listing_groups_episodes_by_stage_and_finds_year_subdirectories() {
    let workspace = Workspace::new();
    for (stage, base) in [
        ("in-planning", "2026.09.15-planned"),
        ("in-progress", "2026.08.01-recording"),
        ("published/2025", "2025.03.02-back-catalogue"),
        ("published", "not-an-episode-folder"),
    ] {
        fs::create_dir_all(workspace.root.join("episodes").join(stage).join(base)).unwrap();
    }

    let output = workspace
        .eps(&["--format", "json", "episode", "list"])
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let episodes = report["episodes"].as_array().unwrap();
    let bases = episodes
        .iter()
        .map(|episode| episode["base"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        bases,
        [
            "2026.09.15-planned",
            "2026.08.01-recording",
            "2025.03.02-back-catalogue"
        ]
    );
    assert_eq!(episodes[2]["date"], "2025-03-02");
    assert_eq!(episodes[0]["has_metadata"], false);

    let output = workspace
        .eps(&[
            "--format",
            "json",
            "episode",
            "list",
            "--stage",
            "in-progress",
        ])
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["episodes"].as_array().unwrap().len(), 1);
}

#[test]
fn promotion_is_guarded_and_reversible_only_with_force() {
    let workspace = Workspace::new();
    workspace
        .eps(&["episode", "new", "2026-09-15", "Rural informatics"])
        .unwrap();
    let planned = workspace
        .root
        .join("episodes/in-planning/2026.09.15-rural-informatics");

    workspace
        .eps(&[
            "episode",
            "promote",
            planned.to_str().unwrap(),
            "in-progress",
        ])
        .unwrap();
    assert!(!planned.exists());
    let recording = workspace
        .root
        .join("episodes/in-progress/2026.09.15-rural-informatics");
    assert!(
        recording
            .join("2026.09.15-rural-informatics.episode-data.md")
            .is_file()
    );

    // Publishing an episode that has never been uploaded would misrepresent it.
    let output = workspace.run(&[
        "episode",
        "promote",
        recording.to_str().unwrap(),
        "published",
    ]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("no Video ID recorded")
    );
    assert!(recording.exists());

    // Going backwards is possible, but only deliberately.
    let output = workspace.run(&[
        "episode",
        "promote",
        recording.to_str().unwrap(),
        "in-planning",
    ]);
    assert!(!output.status.success());
    workspace
        .eps(&[
            "episode",
            "promote",
            recording.to_str().unwrap(),
            "in-planning",
            "--force",
        ])
        .unwrap();
    assert!(planned.exists());
}

#[test]
fn a_promotion_dry_run_moves_nothing() {
    let workspace = Workspace::new();
    workspace
        .eps(&["episode", "new", "2026-09-15", "Rural informatics"])
        .unwrap();
    let planned = workspace
        .root
        .join("episodes/in-planning/2026.09.15-rural-informatics");

    let output = workspace
        .eps(&[
            "--dry-run",
            "episode",
            "promote",
            planned.to_str().unwrap(),
            "in-progress",
        ])
        .unwrap();
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Would promote")
    );
    assert!(planned.exists());
    assert!(
        !workspace
            .root
            .join("episodes/in-progress/2026.09.15-rural-informatics")
            .exists()
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

/// A workspace laid out the way a real channel workspace is: a channel config
/// under `channels/`, with episodes resolved relative to the workspace root.
struct Workspace {
    _temp: TempDir,
    root: PathBuf,
    config: PathBuf,
}

impl Workspace {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        fs::create_dir_all(root.join("channels")).unwrap();
        let config = root.join("channels/fixture-channel.toml");
        fs::write(
            &config,
            r#"
[channel]
id = "fixture-channel"
name = "Fixture Channel"
host = "Fixture Host"
timezone = "Europe/London"

[paths]
episodes = "episodes"

[youtube]
channel_id = "UCfixturefixturefixtur"
default_privacy = "unlisted"
language = "en-GB"

[playlists]
binge_watch_id = "PLfixturebinge"
audio_only_id = "PLfixtureaudio"

[recording.vdo_ninja]
guest_url = "https://example.invalid/guest"
host_url = "https://example.invalid/host"

[archive.forum]
topic_url = "https://forum.invalid/archive"
"#,
        )
        .unwrap();
        Self {
            _temp: temp,
            root,
            config,
        }
    }

    fn run(&self, arguments: &[&str]) -> std::process::Output {
        cargo_bin_cmd!("eps")
            .arg("--config")
            .arg(&self.config)
            .args(arguments)
            .current_dir(&self.root)
            .output()
            .unwrap()
    }

    fn eps(&self, arguments: &[&str]) -> Result<std::process::Output, String> {
        let output = self.run(arguments);
        if output.status.success() {
            Ok(output)
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
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
