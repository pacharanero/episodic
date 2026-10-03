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
fn config_selection_uses_pwd_or_explicit_path_and_reports_one_channel() {
    let workspace = Workspace::new();
    let elsewhere = workspace.root.join("elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    // An invalid local config must not interfere with an explicit selector.
    fs::write(elsewhere.join("eps.toml"), "invalid TOML").unwrap();
    for (cwd, selector) in [
        (&workspace.root, None),
        (&elsewhere, Some(PathBuf::from("./../eps.toml"))),
        (&elsewhere, Some(workspace.config.clone())),
    ] {
        for command in ["show", "list"] {
            let mut cmd = cargo_bin_cmd!("eps");
            cmd.current_dir(cwd);
            if let Some(path) = &selector {
                cmd.arg("--config").arg(path);
            }
            let output = cmd
                .args(["channel", command, "--format", "json"])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let value: Value = serde_json::from_slice(&output.stdout).unwrap();
            let summary = if command == "list" {
                assert_eq!(value.as_array().unwrap().len(), 1);
                &value[0]
            } else {
                &value
            };
            assert_eq!(summary["id"], "fixture-channel");
            assert_eq!(summary["config_path"], workspace.config.to_str().unwrap());
        }
        let mut cmd = cargo_bin_cmd!("eps");
        cmd.current_dir(cwd);
        if let Some(path) = &selector {
            cmd.arg("--config").arg(path);
        }
        let output = cmd
            .args([
                "episode",
                "new",
                "2026-09-07",
                "Portable",
                "--dry-run",
                "--format",
                "json",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(
                workspace
                    .root
                    .join("episodes/in-planning/2026.09.07-portable")
                    .to_str()
                    .unwrap()
            )
        );
        assert!(!workspace.root.join("episodes").exists());
    }
}

#[test]
fn missing_and_invalid_selected_configs_never_fall_back() {
    let workspace = Workspace::new();
    let child = workspace.root.join("child");
    fs::create_dir(&child).unwrap();
    let legacy = child.join("channels/fixture-channel");
    fs::create_dir_all(&legacy).unwrap();
    fs::copy(&workspace.config, legacy.join("channel.toml")).unwrap();
    let global = child.join("global/eps");
    fs::create_dir_all(&global).unwrap();
    fs::copy(&workspace.config, global.join("eps.toml")).unwrap();
    // No ancestor traversal, legacy discovery, environment or global fallback.
    let output = cargo_bin_cmd!("eps")
        .current_dir(&child)
        .env("EPS_CONFIG", &workspace.config)
        .env("XDG_CONFIG_HOME", child.join("global"))
        .env("HOME", &child)
        .args(["channel", "show"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(child.join("eps.toml").to_str().unwrap())
    );

    let bad = workspace.root.join("bad.toml");
    let valid = fs::read_to_string(&workspace.config).unwrap();
    for (source, diagnostic) in [
        (None, "could not resolve"),
        (Some("[channel".to_owned()), "could not parse"),
        (
            Some(valid.replace("fixture-channel", "INVALID_ID")),
            "channel.id",
        ),
        (Some(format!("{valid}\n[unknown]\nvalue = 1\n")), "unknown"),
        (Some(valid.replace("host =", "typo =")), "typo"),
    ] {
        if let Some(source) = &source {
            fs::write(&bad, source).unwrap();
        }
        for command in ["show", "list", "validate"] {
            let output = cargo_bin_cmd!("eps")
                .current_dir(&workspace.root)
                .args(["--config", "bad.toml", "channel", command])
                .output()
                .unwrap();
            assert!(!output.status.success());
            let messages = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(messages.contains(diagnostic), "{messages}");
            assert!(messages.contains(bad.to_str().unwrap()), "{messages}");
        }
        if let Some(source) = source {
            fs::write(child.join("eps.toml"), source).unwrap();
            let output = cargo_bin_cmd!("eps")
                .current_dir(&child)
                .args(["episode", "new", "2026-09-07", "Must not create"])
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(!child.join("episodes").exists());
        }
    }
    let output = cargo_bin_cmd!("eps")
        .current_dir(&workspace.root)
        .args(["--channel", "fixture-channel", "channel", "show"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[cfg(unix)]
#[test]
fn config_normalization_preserves_symlink_and_parent_traversal_semantics() {
    let workspace = Workspace::new();
    let caller = tempfile::tempdir().unwrap();
    fs::create_dir(workspace.root.join("child")).unwrap();
    std::os::unix::fs::symlink(
        workspace.root.join("child"),
        caller.path().join("linked-child"),
    )
    .unwrap();
    std::os::unix::fs::symlink(&workspace.config, caller.path().join("linked.toml")).unwrap();
    for selector in ["linked-child/../eps.toml", "linked.toml"] {
        let output = cargo_bin_cmd!("eps")
            .current_dir(caller.path())
            .args(["--config", selector, "channel", "show", "--format", "json"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["config_path"], workspace.config.to_str().unwrap());
    }
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
        "- [x] Remixing allowed (channel-wide policy; supports Shorts and publicity)",
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

/// Write an executable stand-in for an external tool.
fn fake(directory: &Path, name: &str, script: &str) -> PathBuf {
    let path = directory.join(name);
    fs::write(&path, format!("#!/usr/bin/env bash\n{script}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

/// ffprobe that accepts anything non-empty, so tests control readability by
/// controlling file contents.
const FAKE_FFPROBE: &str = r#"
for argument in "$@"; do :; done
target="${!#}"
if [ "$1" = "-v" ] && [ -n "$EPISODIC_TEST_PROBE_FAILS" ] && [[ "$target" == *staging* ]]; then
  exit 1
fi
case " $* " in
  *" stream=codec_type "*) echo "$EPISODIC_TEST_STREAMS" ;;
  *" format=duration "*) echo "${EPISODIC_TEST_DURATION:-1305.6}" ;;
esac
[ -s "$target" ] || exit 1
"#;

#[test]
fn a_tool_that_exits_successfully_but_writes_rubbish_publishes_nothing() {
    let workspace = Workspace::new();
    let tools = workspace.root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    let input = workspace.root.join("episode.mp4");
    fs::write(&input, "input media").unwrap();

    // ffmpeg "succeeds" and writes a file that ffprobe then rejects.
    let ffmpeg = fake(&tools, "ffmpeg", r#"printf 'garbage' > "${!#}"; exit 0"#);
    let ffprobe = fake(&tools, "ffprobe", FAKE_FFPROBE);

    let output = cargo_bin_cmd!("eps")
        .arg("--config")
        .arg(&workspace.config)
        .args(["transcript", "extract", input.to_str().unwrap()])
        .env("EPISODIC_FFMPEG_BIN", &ffmpeg)
        .env("EPISODIC_FFPROBE_BIN", &ffprobe)
        .env("EPISODIC_TEST_PROBE_FAILS", "1")
        .current_dir(&workspace.root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("unreadable output")
    );
    assert!(!workspace.root.join("episode.wav").exists());
    // The staged file must be cleaned up, not left to be mistaken for work.
    let leftovers = fs::read_dir(&workspace.root)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains("staging"))
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn a_failing_tool_leaves_no_artefact_behind() {
    let workspace = Workspace::new();
    let tools = workspace.root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    let input = workspace.root.join("episode.mp4");
    fs::write(&input, "input media").unwrap();

    let ffmpeg = fake(
        &tools,
        "ffmpeg",
        r#"echo "codec not supported" >&2; exit 1"#,
    );
    let ffprobe = fake(&tools, "ffprobe", FAKE_FFPROBE);

    let output = cargo_bin_cmd!("eps")
        .arg("--config")
        .arg(&workspace.config)
        .args(["transcript", "extract", input.to_str().unwrap()])
        .env("EPISODIC_FFMPEG_BIN", &ffmpeg)
        .env("EPISODIC_FFPROBE_BIN", &ffprobe)
        .current_dir(&workspace.root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    // The tool's own diagnostics are the useful part, so they must survive.
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("codec not supported")
    );
    assert!(!workspace.root.join("episode.wav").exists());
}

/// The documented pipeline runs on the processed master, so every later
/// artefact used to inherit a `.processed` infix that `episode validate` and
/// the YouTube caption lookup did not expect. A full run must produce the
/// names the artefact contract declares (F2).
#[test]
fn the_documented_pipeline_produces_the_names_the_contract_declares() {
    let workspace = Workspace::new();
    let tools = workspace.root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    let base = "2026.09.15-naming";
    let episode = workspace.root.join("episodes/in-progress").join(base);
    fs::create_dir_all(&episode).unwrap();
    fs::write(
        episode.join(format!("{base}.episode-data.md")),
        format!("# Episode Data: Naming\n\n## Episode Details\n\n- **Episode Date**: 2026-09-15\n- **Title**: Naming\n- **Video ID**: [TBD]\n\n## Files\n\n- **Edited Master**: `{base}.mp4`\n"),
    )
    .unwrap();
    let master = episode.join(format!("{base}.mp4"));
    fs::write(&master, "edited master").unwrap();

    // Writes to the last argument, and reports loudness when measuring.
    let ffmpeg = fake(
        &tools,
        "ffmpeg",
        r#"
case " $* " in
  *" -f null "*)
    echo '{"input_i":"-19.0","input_tp":"-2.0","input_lra":"6.0","input_thresh":"-29.0"}' >&2
    exit 0 ;;
esac
printf 'media' > "${!#}"
"#,
    );
    let ffprobe = fake(&tools, "ffprobe", FAKE_FFPROBE);
    let whisper = fake(
        &tools,
        "whisper",
        r#"
input="$1"
out=""
while [ $# -gt 0 ]; do
  if [ "$1" = "--output_dir" ]; then out="$2"; fi
  shift
done
stem="$(basename "$input")"
stem="${stem%.*}"
printf '1\n00:00:00,000 --> 00:00:10,000\nIntroduction\n' > "$out/$stem.srt"
for extension in vtt txt json tsv; do printf 'content\n' > "$out/$stem.$extension"; done
"#,
    );

    let run = |arguments: &[&str]| {
        let output = cargo_bin_cmd!("eps")
            .arg("--config")
            .arg(&workspace.config)
            .args(arguments)
            .env("EPISODIC_FFMPEG_BIN", &ffmpeg)
            .env("EPISODIC_FFPROBE_BIN", &ffprobe)
            .env("EPISODIC_WHISPER_BIN", &whisper)
            .env("EPISODIC_TEST_STREAMS", "video\naudio")
            .current_dir(&workspace.root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{:?} failed: {}",
            arguments,
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };

    let processed = episode.join(format!("{base}.processed.mp4"));
    let wav = episode.join(format!("{base}.wav"));
    run(&["audio", "process", master.to_str().unwrap()]);
    assert!(processed.is_file(), "the upload master keeps its infix");
    run(&["transcript", "extract", processed.to_str().unwrap()]);
    assert!(
        wav.is_file(),
        "extracted audio is <base>.wav, not <base>.processed.wav"
    );
    run(&["transcript", "transcribe", wav.to_str().unwrap()]);

    for extension in ["srt", "vtt", "txt", "json", "tsv"] {
        assert!(
            episode.join(format!("{base}.{extension}")).is_file(),
            "the transcript family must be <base>.{extension}"
        );
    }
    assert!(
        fs::read_dir(&episode)
            .unwrap()
            .flatten()
            .all(
                |entry| entry.file_name().to_string_lossy() == format!("{base}.processed.mp4")
                    || !entry.file_name().to_string_lossy().contains(".processed.")
            ),
        "only the upload master may carry the .processed infix"
    );

    // The real test of the naming: the validator has to find the captions.
    let report = run(&[
        "--format",
        "json",
        "episode",
        "validate",
        episode.to_str().unwrap(),
    ]);
    let report: Value = serde_json::from_slice(&report.stdout).unwrap();
    let findings = report["findings"].as_array().unwrap();
    assert!(
        !findings
            .iter()
            .any(|finding| finding["code"] == "missing-captions"),
        "a complete pipeline run must not report missing captions: {findings:?}"
    );
}

/// Transcribing the processed master directly must still publish the clean
/// names, because the file is there to be transcribed and operators will
/// reach for it.
#[test]
fn transcribing_the_processed_master_still_publishes_the_clean_family() {
    let workspace = Workspace::new();
    let tools = workspace.root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    let input = workspace.root.join("episode.processed.wav");
    fs::write(&input, "audio").unwrap();

    let whisper = fake(
        &tools,
        "whisper",
        r#"
input="$1"
out=""
while [ $# -gt 0 ]; do
  if [ "$1" = "--output_dir" ]; then out="$2"; fi
  shift
done
stem="$(basename "$input")"
stem="${stem%.*}"
printf '1\n00:00:00,000 --> 00:00:01,000\nhello\n' > "$out/$stem.srt"
for extension in vtt txt json tsv; do printf 'content\n' > "$out/$stem.$extension"; done
"#,
    );
    let ffprobe = fake(&tools, "ffprobe", FAKE_FFPROBE);

    let output = cargo_bin_cmd!("eps")
        .arg("--config")
        .arg(&workspace.config)
        .args(["transcript", "transcribe", input.to_str().unwrap()])
        .env("EPISODIC_WHISPER_BIN", &whisper)
        .env("EPISODIC_FFPROBE_BIN", &ffprobe)
        .current_dir(&workspace.root)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for extension in ["srt", "vtt", "txt", "json", "tsv"] {
        assert!(
            workspace
                .root
                .join(format!("episode.{extension}"))
                .is_file(),
            "expected episode.{extension}"
        );
        assert!(
            !workspace
                .root
                .join(format!("episode.processed.{extension}"))
                .exists(),
            "episode.processed.{extension} must not be published"
        );
    }
}

/// Whisper's trailing-silence failure mode, as observed: a real closing
/// sentence, then eight `Bye.` and two `Thank you.` over a silent tail, the
/// last ending 29 seconds past the end of the media (F3).
struct HallucinatedTranscript {
    _temp: TempDir,
    source: PathBuf,
}

impl HallucinatedTranscript {
    const CUES: usize = 11;
    const DURATION: f64 = 1305.6;

    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().to_path_buf();
        let mut cues = vec![(
            1240.0,
            1244.74,
            "and that is where the evidence runs out".to_owned(),
        )];
        for index in 0..8 {
            let start = 1255.0 + f64::from(index) * 5.0;
            cues.push((start, start + 5.0, "Bye.".to_owned()));
        }
        cues.push((1295.0, 1325.0, "Thank you.".to_owned()));
        cues.push((1305.88, 1334.88, "Thank you.".to_owned()));
        assert_eq!(cues.len(), Self::CUES);

        let stamp = |seconds: f64, fraction: char| {
            let millis = (seconds * 1000.0).round() as u64;
            format!(
                "{:02}:{:02}:{:02}{fraction}{:03}",
                millis / 3_600_000,
                millis / 60_000 % 60,
                millis / 1000 % 60,
                millis % 1000
            )
        };

        let mut srt = String::new();
        let mut vtt = "WEBVTT\n\n".to_owned();
        let mut txt = String::new();
        let mut tsv = "start\tend\ttext\n".to_owned();
        let mut segments = Vec::new();
        for (index, (start, end, text)) in cues.iter().enumerate() {
            srt.push_str(&format!(
                "{}\n{} --> {}\n{text}\n\n",
                index + 1,
                stamp(*start, ','),
                stamp(*end, ',')
            ));
            vtt.push_str(&format!(
                "{} --> {}\n{text}\n\n",
                stamp(*start, '.'),
                stamp(*end, '.')
            ));
            txt.push_str(&format!("{text}\n"));
            tsv.push_str(&format!(
                "{}\t{}\t{text}\n",
                (start * 1000.0) as u64,
                (end * 1000.0) as u64
            ));
            segments.push(serde_json::json!({
                "id": index,
                "start": start,
                "end": end,
                "text": format!(" {text}"),
                "tokens": [index],
            }));
        }
        let json = serde_json::json!({
            "text": segments.iter().filter_map(|s| s["text"].as_str()).collect::<String>(),
            "segments": segments,
            "language": "en",
        });

        fs::write(source.join("transcript.srt"), srt).unwrap();
        fs::write(source.join("transcript.vtt"), vtt).unwrap();
        fs::write(source.join("transcript.txt"), txt).unwrap();
        fs::write(source.join("transcript.tsv"), tsv).unwrap();
        fs::write(
            source.join("transcript.json"),
            serde_json::to_string(&json).unwrap(),
        )
        .unwrap();

        Self {
            _temp: temp,
            source,
        }
    }
}

/// Copies a prepared family into Whisper's output directory.
const FAKE_WHISPER_COPY: &str = r#"
input="$1"
out=""
while [ $# -gt 0 ]; do
  if [ "$1" = "--output_dir" ]; then out="$2"; fi
  shift
done
stem="$(basename "$input")"
stem="${stem%.*}"
for extension in srt vtt txt json tsv; do
  cp "$EPISODIC_TEST_TRANSCRIPT/transcript.$extension" "$out/$stem.$extension"
done
"#;

/// Reports a peak level when asked to measure, and writes to its output
/// otherwise.
const FAKE_FFMPEG_VOLUME: &str = r#"
case " $* " in
  *volumedetect*)
    echo "[Parsed_volumedetect_0 @ 0x0] max_volume: ${EPISODIC_TEST_PEAK:--91.0} dB" >&2
    exit 0 ;;
esac
printf 'media' > "${!#}"
"#;

fn transcribe_hallucinated(peak: &str) -> (TempDir, PathBuf, std::process::Output) {
    let transcript = HallucinatedTranscript::new();
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_path_buf();
    fs::write(
        root.join("eps.toml"),
        "[channel]\nid = \"c\"\nname = \"C\"\ntimezone = \"Europe/London\"\n",
    )
    .unwrap();
    let tools = root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    let input = root.join("episode.wav");
    fs::write(&input, "audio").unwrap();

    let output = cargo_bin_cmd!("eps")
        .args(["transcript", "transcribe", input.to_str().unwrap()])
        .env(
            "EPISODIC_WHISPER_BIN",
            fake(&tools, "whisper", FAKE_WHISPER_COPY),
        )
        .env(
            "EPISODIC_FFPROBE_BIN",
            fake(&tools, "ffprobe", FAKE_FFPROBE),
        )
        .env(
            "EPISODIC_FFMPEG_BIN",
            fake(&tools, "ffmpeg", FAKE_FFMPEG_VOLUME),
        )
        .env("EPISODIC_TEST_TRANSCRIPT", &transcript.source)
        .env(
            "EPISODIC_TEST_DURATION",
            HallucinatedTranscript::DURATION.to_string(),
        )
        .env("EPISODIC_TEST_PEAK", peak)
        .current_dir(&root)
        .output()
        .unwrap();

    (temp, root, output)
}

#[test]
fn hallucinated_cues_over_a_silent_tail_are_removed_from_every_format() {
    let (_temp, root, output) = transcribe_hallucinated("-91.0");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let srt = fs::read_to_string(root.join("episode.srt")).unwrap();
    assert!(
        srt.contains("and that is where the evidence runs out"),
        "real speech must survive"
    );
    assert!(!srt.contains("Bye."), "repeats over silence must go: {srt}");
    assert!(!srt.contains("Thank you."));

    // Five transcripts that disagree about where the episode ends would be
    // worse than one that is too long, so every format is corrected together.
    let vtt = fs::read_to_string(root.join("episode.vtt")).unwrap();
    assert!(vtt.starts_with("WEBVTT"));
    assert!(!vtt.contains("Bye."));
    assert_eq!(
        fs::read_to_string(root.join("episode.txt")).unwrap(),
        "and that is where the evidence runs out\n"
    );
    let tsv = fs::read_to_string(root.join("episode.tsv")).unwrap();
    assert!(tsv.starts_with("start\tend\ttext\n"));
    assert_eq!(tsv.lines().count(), 2, "header plus the one real cue");
    let json: Value =
        serde_json::from_str(&fs::read_to_string(root.join("episode.json")).unwrap()).unwrap();
    assert_eq!(json["segments"].as_array().unwrap().len(), 1);
    assert_eq!(json["language"], "en");
    assert_eq!(json["text"], " and that is where the evidence runs out");

    // Removing published state silently would be worse than leaving it.
    let reported = String::from_utf8_lossy(&output.stderr);
    assert!(
        reported.contains("removed 10 of 11 cues") && reported.contains("over silence"),
        "what was removed must be reported: {reported}"
    );
}

#[test]
fn repeated_cues_over_real_audio_are_kept_and_the_doubt_reported() {
    // The same text over audible speech is someone actually saying it twice,
    // so only the cue beyond the end of the media may be removed.
    let (_temp, root, output) = transcribe_hallucinated("-14.2");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let srt = fs::read_to_string(root.join("episode.srt")).unwrap();
    assert_eq!(
        srt.matches("Bye.").count(),
        8,
        "repeats over real audio must be kept"
    );
    assert_eq!(
        srt.matches("Thank you.").count(),
        1,
        "only the cue starting past the end of the media goes"
    );
    let reported = String::from_utf8_lossy(&output.stderr);
    assert!(
        reported.contains("not silent") && reported.contains("starting after the end of the media"),
        "the doubt must be reported: {reported}"
    );
}

#[test]
fn a_partial_transcript_family_is_never_published() {
    let workspace = Workspace::new();
    let tools = workspace.root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    let input = workspace.root.join("episode.wav");
    fs::write(&input, "audio").unwrap();

    // Whisper writes the srt and txt but dies before the rest.
    let whisper = fake(
        &tools,
        "whisper",
        r#"
out=""
while [ $# -gt 0 ]; do
  if [ "$1" = "--output_dir" ]; then out="$2"; fi
  shift
done
printf '1
00:00:00,000 --> 00:00:01,000
hello
' > "$out/episode.srt"
printf 'hello
' > "$out/episode.txt"
exit 0
"#,
    );

    let output = cargo_bin_cmd!("eps")
        .arg("--config")
        .arg(&workspace.config)
        .args(["transcript", "transcribe", input.to_str().unwrap()])
        .env("EPISODIC_WHISPER_BIN", &whisper)
        .current_dir(&workspace.root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("did not produce a non-empty vtt")
    );
    // Nothing at all is published: a truncated SRT would silently become the
    // input to chapters and captions.
    for extension in ["srt", "vtt", "txt", "json", "tsv"] {
        assert!(
            !workspace.root.join(format!("episode.{extension}")).exists(),
            "published {extension} from a partial run"
        );
    }
}

#[test]
fn a_media_dry_run_reports_the_real_command_and_writes_nothing() {
    let workspace = Workspace::new();
    let tools = workspace.root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    let input = workspace.root.join("episode.mp4");
    fs::write(&input, "input media").unwrap();

    let ffmpeg = fake(&tools, "ffmpeg", "exit 1");
    let ffprobe = fake(&tools, "ffprobe", FAKE_FFPROBE);

    let output = cargo_bin_cmd!("eps")
        .arg("--config")
        .arg(&workspace.config)
        .args([
            "--dry-run",
            "transcript",
            "extract",
            input.to_str().unwrap(),
        ])
        .env("EPISODIC_FFMPEG_BIN", &ffmpeg)
        .env("EPISODIC_FFPROBE_BIN", &ffprobe)
        .current_dir(&workspace.root)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Would write"));
    assert!(stdout.contains("-ar 16000"));
    assert!(stdout.contains("pcm_s16le"));
    assert!(!workspace.root.join("episode.wav").exists());
}

/// A workspace with a prompt library and a scripted model.
fn metadata_workspace(model_script: &str) -> (Workspace, PathBuf) {
    let workspace = Workspace::new();
    fs::create_dir_all(workspace.root.join("prompts")).unwrap();
    fs::write(
        workspace.root.join("prompts/chapters.md"),
        "Chapters for {{channel.name}}.\n\n{{transcript}}\n",
    )
    .unwrap();

    let tools = workspace.root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    let model = fake(&tools, "fake-model", model_script);

    // Route the chapters task at a cli profile running the scripted model.
    let mut config = fs::read_to_string(&workspace.config).unwrap();
    config.push_str(&format!(
        "\n[llm]\ndefault_profile = \"test\"\n\n[llm.profiles.test]\nprovider = \"cli\"\ncommand = \"{}\"\nmodel = \"scripted\"\ntimeout_seconds = 30\n\n[llm.tasks]\nchapters = \"test\"\n",
        model.display()
    ));
    fs::write(&workspace.config, config).unwrap();

    let transcript = workspace.root.join("episode.srt");
    fs::write(
        &transcript,
        "1\n00:00:00,000 --> 00:00:10,000\nopening words\n\n2\n00:06:00,000 --> 00:06:30,000\nclosing words\n",
    )
    .unwrap();
    (workspace, transcript)
}

/// Twelve valid chapter lines reaching the end of the fixture transcript.
const VALID_CHAPTERS: &str = "0:00 Introduction\n1:00 A chapter about things\n2:00 Another chapter here\n3:00 A third chapter now\n4:00 A fourth chapter here\n5:00 A fifth chapter here\n6:00 A sixth chapter here";

#[test]
fn an_invalid_answer_is_retried_and_the_failure_reported() {
    // A well-formed list with the wrong opening line: the shape is right, so
    // only the task contract catches it.
    let wrong_first_line = VALID_CHAPTERS.replacen("0:00 Introduction", "Introduction at 0:00", 1);
    let script = format!(
        "count_file=\"$(dirname \"$0\")/attempts\"\n\
         count=$(cat \"$count_file\" 2>/dev/null || echo 0)\n\
         echo $((count + 1)) > \"$count_file\"\n\
         if [ \"$count\" -eq 0 ]; then\n\
         cat <<'FIRST'\n{wrong_first_line}\nFIRST\n\
         else\n\
         cat <<'SECOND'\n{VALID_CHAPTERS}\nSECOND\n\
         fi\n"
    );
    let (workspace, transcript) = metadata_workspace(&script);
    let output = workspace.root.join("chapters.txt");

    let result = cargo_bin_cmd!("eps")
        .arg("--config")
        .arg(&workspace.config)
        .args([
            "--format",
            "json",
            "metadata",
            "chapters",
            transcript.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .current_dir(&workspace.root)
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["attempts"], 2);
    assert_eq!(report["applied"], true);
    let rejected = report["rejected"].as_array().unwrap();
    assert_eq!(rejected.len(), 1);
    assert!(
        rejected[0].as_str().unwrap().contains("first chapter"),
        "unexpected rejection reason: {}",
        rejected[0]
    );
    assert!(
        fs::read_to_string(&output)
            .unwrap()
            .starts_with("0:00 Introduction")
    );
}

#[test]
fn a_model_that_never_satisfies_the_contract_writes_nothing() {
    let (workspace, transcript) = metadata_workspace("printf 'not chapters at all\n'");
    let output = workspace.root.join("chapters.txt");

    let result = cargo_bin_cmd!("eps")
        .arg("--config")
        .arg(&workspace.config)
        .args([
            "metadata",
            "chapters",
            transcript.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .current_dir(&workspace.root)
        .output()
        .unwrap();

    assert!(!result.status.success());
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(stderr.contains("after 3 attempts"), "{stderr}");
    // The whole point: a model failure must not leave a completed artefact.
    assert!(!output.exists());
}

#[test]
fn an_empty_model_response_is_a_failure_not_an_artefact() {
    let (workspace, transcript) = metadata_workspace("exit 0");
    let output = workspace.root.join("chapters.txt");

    let result = cargo_bin_cmd!("eps")
        .arg("--config")
        .arg(&workspace.config)
        .args([
            "metadata",
            "chapters",
            transcript.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .current_dir(&workspace.root)
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("empty output")
    );
    assert!(!output.exists());
}

#[test]
fn generated_output_is_not_silently_regenerated() {
    let (workspace, transcript) = metadata_workspace(&format!("printf '{VALID_CHAPTERS}\n'"));
    let output = workspace.root.join("chapters.txt");
    fs::write(&output, "human edited chapters\n").unwrap();

    let result = cargo_bin_cmd!("eps")
        .arg("--config")
        .arg(&workspace.config)
        .args([
            "--format",
            "json",
            "metadata",
            "chapters",
            transcript.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .current_dir(&workspace.root)
        .output()
        .unwrap();

    assert!(result.status.success());
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["skipped"], true);
    assert_eq!(
        fs::read_to_string(&output).unwrap(),
        "human edited chapters\n",
        "an existing artefact must not be overwritten without --force"
    );
}

#[test]
fn an_existing_artefact_can_be_checked_without_a_model() {
    let (workspace, transcript) = metadata_workspace("exit 1");
    let artefact = workspace.root.join("chapters.txt");
    fs::write(&artefact, format!("{VALID_CHAPTERS}\n")).unwrap();

    let ok = cargo_bin_cmd!("eps")
        .arg("--config")
        .arg(&workspace.config)
        .args([
            "metadata",
            "check",
            "chapters",
            artefact.to_str().unwrap(),
            "--transcript",
            transcript.to_str().unwrap(),
        ])
        .current_dir(&workspace.root)
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );

    fs::write(&artefact, "0:00 Introduction\n").unwrap();
    let bad = cargo_bin_cmd!("eps")
        .arg("--config")
        .arg(&workspace.config)
        .args(["metadata", "check", "chapters", artefact.to_str().unwrap()])
        .current_dir(&workspace.root)
        .output()
        .unwrap();
    assert!(!bad.status.success());
}

#[test]
fn frame_extraction_never_overwrites_a_frame_already_chosen() {
    let workspace = Workspace::new();
    let tools = workspace.root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    let episode = workspace.root.join("2026.08.07-fixture");
    fs::create_dir_all(&episode).unwrap();
    let video = episode.join("2026.08.07-fixture.processed.mp4");
    fs::write(&video, "video").unwrap();

    let ffmpeg = fake(&tools, "ffmpeg", r#"printf 'frame' > "${!#}"; exit 0"#);
    let ffprobe = fake(&tools, "ffprobe", "echo 300.0");

    // A frame the human has already replaced with their own crop.
    let frames = episode.join("2026.08.07-fixture.frames");
    fs::create_dir_all(&frames).unwrap();
    fs::write(frames.join("frame-02.jpg"), "the chosen one").unwrap();

    let output = cargo_bin_cmd!("eps")
        .arg("--config")
        .arg(&workspace.config)
        .args([
            "--format",
            "json",
            "thumbnail",
            "frames",
            video.to_str().unwrap(),
            "--count",
            "3",
        ])
        .env("EPISODIC_FFMPEG_BIN", &ffmpeg)
        .env("EPISODIC_FFPROBE_BIN", &ffprobe)
        .current_dir(&workspace.root)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["outputs"].as_array().unwrap().len(), 3);
    assert_eq!(
        fs::read_to_string(frames.join("frame-02.jpg")).unwrap(),
        "the chosen one",
        "an existing frame must never be regenerated"
    );
    assert!(frames.join("frame-01.jpg").is_file());
    assert!(frames.join("frame-03.jpg").is_file());
}

#[test]
fn composing_reports_a_missing_font_rather_than_a_broken_thumbnail() {
    let workspace = Workspace::new();
    let image = workspace.root.join("frame.jpg");
    fs::write(&image, "image").unwrap();

    let mut config = fs::read_to_string(&workspace.config).unwrap();
    config
        .push_str("\n[thumbnail]\nwidth = 1280\nheight = 720\nfont_path = \"fonts/Missing.ttf\"\n");
    fs::write(&workspace.config, config).unwrap();

    let output = cargo_bin_cmd!("eps")
        .arg("--config")
        .arg(&workspace.config)
        .args([
            "thumbnail",
            "compose",
            image.to_str().unwrap(),
            "Some caption text",
        ])
        .current_dir(&workspace.root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("font not found")
    );
    // Named after the episode directory rather than the frame, so look for any
    // member of the pair rather than one guessed name.
    assert!(
        fs::read_dir(&workspace.root)
            .unwrap()
            .flatten()
            .all(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                !name.ends_with(".thumbnail.png") && !name.ends_with(".thumbnail.svg")
            }),
        "a missing font must leave no thumbnail behind"
    );
}

/// Choosing a frame and a caption is iterative, so recomposing must be
/// possible without deleting the pair by hand. The SVG is the one operators
/// miss, so it has to be replaced too.
#[test]
fn recomposing_a_thumbnail_needs_force_and_replaces_both_members() {
    let workspace = Workspace::new();
    let tools = workspace.root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    let font = workspace.root.join("fonts/Font.ttf");
    fs::create_dir_all(font.parent().unwrap()).unwrap();
    fs::write(&font, "font").unwrap();
    let image = workspace.root.join("frame.jpg");
    fs::write(&image, "image").unwrap();

    let mut config = fs::read_to_string(&workspace.config).unwrap();
    config.push_str("\n[thumbnail]\nwidth = 1280\nheight = 720\nfont_path = \"fonts/Font.ttf\"\n");
    fs::write(&workspace.config, config).unwrap();

    // Renders the caption into the tile, then carries the tile through the
    // composite, so the finished PNG records which caption produced it.
    let magick = fake(
        &tools,
        "magick",
        r#"
caption=""
tile=""
for argument in "$@"; do
  case "$argument" in
    caption:*) caption="${argument#caption:}" ;;
    *caption*) [ -f "$argument" ] && tile="$argument" ;;
  esac
done
if [ -n "$caption" ]; then
  printf '%s' "$caption" > "${!#}"
elif [ -n "$tile" ]; then
  cat "$tile" > "${!#}"
else
  printf 'composite' > "${!#}"
fi
"#,
    );

    let png = workspace.root.join("episode.thumbnail.png");
    let svg = workspace.root.join("episode.thumbnail.svg");

    let compose = |caption: &str, force: bool| {
        let mut command = cargo_bin_cmd!("eps");
        command
            .arg("--config")
            .arg(&workspace.config)
            .args(["thumbnail", "compose", image.to_str().unwrap(), caption])
            .arg("--output")
            .arg(&png);
        if force {
            command.arg("--force");
        }
        command
            .env("EPISODIC_MAGICK_BIN", &magick)
            .current_dir(&workspace.root)
            .output()
            .unwrap()
    };

    let first = compose("first caption", false);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(fs::read_to_string(&png).unwrap().contains("FIRST CAPTION"));
    assert!(fs::read_to_string(&svg).unwrap().contains("FIRST CAPTION"));

    let repeated = compose("second caption", false);
    assert!(repeated.status.success());
    assert!(
        fs::read_to_string(&png).unwrap().contains("FIRST CAPTION"),
        "without --force a finished pair must be left alone"
    );
    assert!(
        String::from_utf8_lossy(&repeated.stdout).contains("Already done"),
        "the skip must say so: {}",
        String::from_utf8_lossy(&repeated.stdout)
    );

    let forced = compose("second caption", true);
    assert!(
        forced.status.success(),
        "{}",
        String::from_utf8_lossy(&forced.stderr)
    );
    assert!(fs::read_to_string(&png).unwrap().contains("SECOND CAPTION"));
    assert!(
        fs::read_to_string(&svg).unwrap().contains("SECOND CAPTION"),
        "the SVG is the member operators forget to delete, so force must replace it"
    );
    assert!(
        fs::read_dir(&workspace.root)
            .unwrap()
            .flatten()
            .all(|entry| !entry.file_name().to_string_lossy().contains("staging")),
        "no staging files may survive a successful recompose"
    );
}

#[test]
fn a_planned_episode_is_not_failed_for_having_no_recording_yet() {
    let temp = tempfile::tempdir().unwrap();
    let episode = temp.path().join("episodes/in-planning/2026.09.15-planned");
    fs::create_dir_all(&episode).unwrap();
    fs::write(
        episode.join("2026.09.15-planned.episode-data.md"),
        "# Episode Data: Planned\n\n## Episode Details\n\n- **Episode Date**: 2026-09-15\n- **Title**: Planned\n",
    )
    .unwrap();

    let output = cargo_bin_cmd!("eps")
        .args([
            "--format",
            "json",
            "episode",
            "validate",
            episode.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "a planned episode should validate: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["findings"].as_array().unwrap().len(), 0);
    assert_eq!(report["ready_for_upload"], false);
    assert_eq!(report["next_action"], "Record the episode");
}

#[test]
fn an_episode_in_progress_reports_outstanding_work_without_failing() {
    let fixture = EpisodeFixture::new(false);
    for extension in ["tags.txt", "youtube-description.txt"] {
        fs::remove_file(
            fixture
                .episode
                .join(format!("{}.{extension}", EpisodeFixture::BASE)),
        )
        .unwrap();
    }

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

    // Work still to do is not a failure while an episode is in progress.
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ready_for_upload"], false);
    let findings = report["findings"].as_array().unwrap();
    assert!(findings.iter().all(|f| f["severity"] == "warning"));
    assert!(findings.iter().any(|f| f["code"] == "missing-tags"));
}

#[test]
fn a_published_episode_is_not_judged_on_artefacts_retention_removed() {
    // After publication, retention deletes the production artefacts. Demanding
    // them back would report the retention policy working as a fault.
    let fixture = EpisodeFixture::new(false);
    let published = fixture
        .episode
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("published");
    fs::create_dir_all(&published).unwrap();
    let moved = published.join(EpisodeFixture::BASE);
    fs::rename(&fixture.episode, &moved).unwrap();

    for extension in [
        "processed.mp4",
        "tags.txt",
        "youtube-description.txt",
        "srt",
        "thumbnail.png",
    ] {
        fs::remove_file(moved.join(format!("{}.{extension}", EpisodeFixture::BASE))).unwrap();
    }
    let data = moved.join(format!("{}.episode-data.md", EpisodeFixture::BASE));
    let text = fs::read_to_string(&data)
        .unwrap()
        .replace("- **Video ID**: [To be added]", "- **Video ID**: abc123")
        .replace(
            "- **Privacy Status**: [unlisted/public]",
            "- **Privacy Status**: public",
        );
    fs::write(&data, text).unwrap();

    let output = cargo_bin_cmd!("eps")
        .args([
            "--format",
            "json",
            "episode",
            "validate",
            moved.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "a published episode with cleaned-up artefacts should validate: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["stage"], "published");
    assert_eq!(report["findings"].as_array().unwrap().len(), 0);
    assert_eq!(report["next_action"], "Nothing outstanding");
}

#[test]
fn a_published_episode_still_recorded_as_unlisted_is_reported() {
    let fixture = EpisodeFixture::new(false);
    let published = fixture
        .episode
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("published");
    fs::create_dir_all(&published).unwrap();
    let moved = published.join(EpisodeFixture::BASE);
    fs::rename(&fixture.episode, &moved).unwrap();

    let data = moved.join(format!("{}.episode-data.md", EpisodeFixture::BASE));
    let text = fs::read_to_string(&data).unwrap().replace(
        "- **Privacy Status**: [unlisted/public]",
        "- **Privacy Status**: unlisted (awaiting guest review)",
    );
    fs::write(&data, text).unwrap();

    let output = cargo_bin_cmd!("eps")
        .args([
            "--format",
            "json",
            "episode",
            "inspect",
            moved.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "published-privacy-mismatch")
    );
    assert_eq!(
        report["next_action"],
        "Reconcile the recorded privacy status against the destination"
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

/// A standalone workspace with eps.toml beside its episodes directory.
struct Workspace {
    _temp: TempDir,
    root: PathBuf,
    config: PathBuf,
}

impl Workspace {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let config = root.join("eps.toml");
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
