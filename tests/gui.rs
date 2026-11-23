// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use assert_cmd::cargo::cargo_bin;
use reqwest::blocking::Client;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn gui_serves_only_authenticated_read_only_workspace_data() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("eps.toml");
    std::fs::write(
        &config,
        "[channel]\nid='fixture'\nname='Fixture channel'\ntimezone='Europe/London'\n",
    )
    .unwrap();
    let assets = temp.path().join("frontend");
    std::fs::create_dir_all(assets.join("assets")).unwrap();
    std::fs::write(assets.join("index.html"), "<h1>Workbench</h1>").unwrap();
    std::fs::write(assets.join("assets/app.js"), "console.log('fixture');").unwrap();
    let episode = temp.path().join("episodes/in-planning/2026.11.23-fixture");
    std::fs::create_dir_all(&episode).unwrap();
    // Missing records must stay visible on the board.
    let child = Command::new(cargo_bin!("eps"))
        .arg("--config")
        .arg(&config)
        .arg("gui")
        .arg("--assets")
        .arg(&assets)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server = Running(child);
    let mut url = String::new();
    BufReader::new(server.0.stdout.take().unwrap())
        .read_line(&mut url)
        .unwrap();
    let (base, token) = url
        .trim()
        .split_once('#')
        .expect("GUI prints a private URL");
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    let api = format!("{base}api/workspace");
    assert_eq!(client.get(&api).send().unwrap().status(), 403);
    assert_eq!(
        client
            .get(&api)
            .bearer_auth("wrong")
            .send()
            .unwrap()
            .status(),
        403
    );
    for (header, value) in [
        ("Origin", "https://example.org"),
        ("Origin", "null"),
        ("Host", "attacker.example"),
    ] {
        assert_eq!(
            client
                .get(&api)
                .bearer_auth(token)
                .header(header, value)
                .send()
                .unwrap()
                .status(),
            403
        );
    }
    let response = client.get(&api).bearer_auth(token).send().unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none()
    );
    let value: serde_json::Value = response.json().unwrap();
    assert_eq!(value["channel"]["name"], "Fixture channel");
    assert_eq!(value["episodes"][0]["error"], "No episode record");
    assert_eq!(
        client
            .post(&api)
            .bearer_auth(token)
            .send()
            .unwrap()
            .status(),
        405
    );
    assert_eq!(
        client
            .post(format!("{base}api/commands"))
            .bearer_auth(token)
            .send()
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        client
            .get(format!("{base}assets/app.js"))
            .send()
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .get(format!("{base}eps.toml"))
            .send()
            .unwrap()
            .status(),
        404
    );
    assert!(
        client
            .get(base)
            .send()
            .unwrap()
            .text()
            .unwrap()
            .contains("Workbench")
    );
    assert!(!episode.join("2026.11.23-fixture.episode-data.md").exists());
}

#[test]
fn gui_serves_episode_images_and_previews_only_with_the_token() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("eps.toml");
    std::fs::write(
        &config,
        "[channel]\nid='fixture'\nname='Fixture channel'\ntimezone='Europe/London'\n",
    )
    .unwrap();
    let assets = temp.path().join("frontend");
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::write(assets.join("index.html"), "<h1>Workbench</h1>").unwrap();
    let base = "2026.11.23-fixture";
    let episode = temp.path().join("episodes/in-progress").join(base);
    std::fs::create_dir_all(episode.join(format!("{base}.frames"))).unwrap();
    std::fs::write(
        episode.join(format!("{base}.episode-data.md")),
        "- **Title**: Fixture\n- **Episode Date**: 2026-11-23\n- **Video ID**: dQw4w9WgXcQ\n",
    )
    .unwrap();
    std::fs::write(
        episode.join(format!("{base}.chapters.txt")),
        "0:00 Introduction\n",
    )
    .unwrap();
    let png = b"\x89PNG\r\n\x1a\nfixture".to_vec();
    std::fs::write(episode.join(format!("{base}.thumbnail.png")), &png).unwrap();
    std::fs::write(
        episode.join(format!("{base}.frames/frame-01.jpg")),
        [0xFF, 0xD8, 0xFF, 0xE0],
    )
    .unwrap();

    let child = Command::new(cargo_bin!("eps"))
        .arg("--config")
        .arg(&config)
        .args(["gui", "--assets"])
        .arg(&assets)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server = Running(child);
    let mut url = String::new();
    BufReader::new(server.0.stdout.take().unwrap())
        .read_line(&mut url)
        .unwrap();
    let (root, token) = url.trim().split_once('#').unwrap();
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    let get = |path: &str, authorised: bool| {
        let request = client.get(format!("{root}{path}"));
        let request = if authorised {
            request.bearer_auth(token)
        } else {
            request
        };
        request.send().unwrap()
    };

    let workspace: serde_json::Value = get("api/workspace", true).json().unwrap();
    let entry = &workspace["episodes"][0];
    assert_eq!(entry["frames"], serde_json::json!(["frame-01.jpg"]));
    assert!(entry["thumbnail"].is_string());
    assert_eq!(
        entry["links"]["youtube"][1]["url"],
        "https://studio.youtube.com/video/dQw4w9WgXcQ/edit"
    );
    assert_eq!(entry["report"]["milestones"].as_array().unwrap().len(), 11);
    assert!(workspace["fingerprint"].is_string());

    let thumbnail = format!("api/episodes/{base}/thumbnail");
    assert_eq!(get(&thumbnail, false).status(), 403);
    let response = get(&thumbnail, true);
    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(response.bytes().unwrap().to_vec(), png);
    assert_eq!(
        get(&format!("api/episodes/{base}/frames/frame-01.jpg"), true).headers()["content-type"],
        "image/jpeg"
    );

    let preview: serde_json::Value = get(&format!("api/episodes/{base}/artefacts/chapters"), true)
        .json()
        .unwrap();
    assert_eq!(preview["text"], "0:00 Introduction\n");
    assert_eq!(preview["truncated"], false);

    for path in [
        format!("api/episodes/{base}/artefacts/credentials"),
        format!("api/episodes/{base}/frames/..%2F..%2F..%2Feps.toml"),
        format!("api/episodes/{base}/frames/%2E%2E"),
        "api/episodes/..%2F..%2Feps.toml/thumbnail".to_owned(),
        "api/episodes/2026.11.23-missing/thumbnail".to_owned(),
    ] {
        assert_eq!(get(&path, true).status(), 404, "{path}");
    }
    let fingerprint: serde_json::Value = get("api/fingerprint", true).json().unwrap();
    assert_eq!(fingerprint["fingerprint"], workspace["fingerprint"]);
}
