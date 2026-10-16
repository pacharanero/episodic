// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use assert_cmd::cargo::{cargo_bin, cargo_bin_cmd};
use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::{Value, json};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn mcp_lists_registry_tools_and_rejects_mutation_without_apply() -> anyhow::Result<()> {
    let transport = TokioChildProcess::new(
        tokio::process::Command::new(cargo_bin("eps")).configure(|command| {
            command.arg("mcp");
        }),
    )?;
    let client = ().serve(transport).await?;
    let tools = client.list_all_tools().await?;
    assert!(
        tools
            .iter()
            .any(|tool| tool.name == "eps--episode--inspect")
    );
    assert!(!tools.iter().any(|tool| tool.name == "eps--mcp"));

    let arguments = json!({ "path": "episode", "assignments": ["Title=New"] });
    let mutation = client
        .call_tool(
            CallToolRequestParams::new("eps--episode--set")
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await?;
    assert_eq!(mutation.is_error, Some(true));
    assert!(
        mutation.structured_content.unwrap()["error"]
            .as_str()
            .unwrap()
            .contains("requires apply: true")
    );
    client.cancel().await?;
    Ok(())
}

#[test]
fn mcp_dispatches_an_explicitly_approved_local_write() {
    let directory = tempdir().unwrap();
    let record = directory.path().join("episode.episode-data.md");
    fs::write(
        &record,
        include_str!("fixtures/episode-data/internal.episode-data.md"),
    )
    .unwrap();
    let input = [
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1" }
            }
        }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized", "params": {} }),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "eps--episode--set",
                "arguments": {
                    "path": record,
                    "assignments": ["Title=Updated fixture"],
                    "force": true,
                    "apply": true
                }
            }
        }),
    ]
    .into_iter()
    .map(|message| message.to_string())
    .collect::<Vec<_>>()
    .join("\n");

    let output = cargo_bin_cmd!("eps")
        .arg("mcp")
        .write_stdin(format!("{input}\n"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .find(|message| message["id"] == 2)
            .is_some_and(|message| message["result"]["isError"] == false)
    );
    assert!(
        fs::read_to_string(record)
            .unwrap()
            .contains("**Title**: Updated fixture")
    );
}
