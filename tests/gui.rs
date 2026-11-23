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
