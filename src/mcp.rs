// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model Context Protocol adapter for the command registry.

use crate::cli::Cli;
use crate::command_registry::{CommandDefinition, Registry, Safety};
use anyhow::{Context, Result, bail};
use clap::CommandFactory;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, JsonObject,
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
        ToolAnnotations,
    },
    service::RequestContext,
    transport::stdio,
};
use serde_json::{Map, Value};
use std::process::Command;
use std::sync::Arc;
use std::{
    path::PathBuf,
    pin::Pin,
    task::{Context as TaskContext, Poll},
};
use tokio::{
    io::{AsyncRead, ReadBuf},
    sync::Semaphore,
};

const MAX_FRAME_BYTES: usize = 1024 * 1024;
const MAX_IN_FLIGHT_REQUESTS: usize = 8;

pub fn run(config: Option<PathBuf>, dry_run: bool) -> Result<()> {
    let executable = std::env::current_exe().context("could not locate the eps executable")?;
    let server = Server::new(executable, config, dry_run);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()
        .context("could not start the MCP runtime")?;
    runtime.block_on(async move {
        let (stdin, stdout) = stdio();
        let service = server
            .serve((BoundedReader::new(stdin, MAX_FRAME_BYTES), stdout))
            .await?;
        service.waiting().await?;
        Ok::<(), anyhow::Error>(())
    })
}

struct Server {
    executable: PathBuf,
    config: Option<PathBuf>,
    dry_run: bool,
    registry: Registry,
    in_flight: Arc<Semaphore>,
}

impl Server {
    fn new(executable: PathBuf, config: Option<PathBuf>, dry_run: bool) -> Self {
        Self {
            executable,
            config,
            dry_run,
            registry: Registry::from_clap(&Cli::command()),
            in_flight: Arc::new(Semaphore::new(MAX_IN_FLIGHT_REQUESTS)),
        }
    }

    fn tool(&self, definition: &CommandDefinition) -> Tool {
        let schema = mcp_schema(definition);
        Tool::new(
            tool_name(definition),
            definition.description.clone(),
            Arc::new(schema),
        )
        .with_title(definition.id.clone())
        .with_annotations(annotations(definition.safety))
    }

    fn definition(&self, name: &str) -> Option<&CommandDefinition> {
        self.registry
            .commands()
            .iter()
            .find(|definition| tool_name(definition) == name)
    }

    fn execute(
        executable: PathBuf,
        config: Option<PathBuf>,
        dry_run: bool,
        definition: CommandDefinition,
        arguments: JsonObject,
    ) -> Result<Value> {
        require_apply(&definition, &arguments)?;
        let mut command = Command::new(executable);
        command.arg("--format").arg("json");
        if let Some(config) = config {
            command.arg("--config").arg(config);
        }
        if dry_run {
            command.arg("--dry-run");
        }
        command.args(&definition.path);
        command.args(cli_arguments(&definition, &arguments)?);

        let output = command.output().context("could not start eps command")?;
        let stdout = String::from_utf8(output.stdout).context("eps returned non-UTF-8 output")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            bail!(if stderr.is_empty() {
                format!("eps exited with {}", output.status)
            } else {
                stderr
            });
        }
        serde_json::from_str(&stdout).context("eps did not return JSON output")
    }
}

impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("episodic", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Tools operate on local episode files. Mutating tools require apply: true; remote and destructive tools retain their existing apply safeguards.",
            )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(
            self.registry
                .commands()
                .iter()
                .map(|tool| self.tool(tool))
                .collect(),
        ))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.definition(name)
            .map(|definition| self.tool(definition))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let Some(definition) = self.definition(&request.name).cloned() else {
            return Err(McpError::invalid_params("unknown episodic tool", None));
        };
        let arguments = request.arguments.unwrap_or_default();
        // The permit covers child execution and response construction, preventing
        // unbounded concurrent work while keeping the async runtime responsive.
        let _permit = self
            .in_flight
            .acquire()
            .await
            .map_err(|_| McpError::internal_error("MCP server is shutting down", None))?;
        let executable = self.executable.clone();
        let config = self.config.clone();
        let dry_run = self.dry_run;
        let result = tokio::task::spawn_blocking(move || {
            Self::execute(executable, config, dry_run, definition, arguments)
        })
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        match result {
            Ok(value) => Ok(CallToolResponse::Complete(CallToolResult::structured(
                value,
            ))),
            Err(error) => Ok(CallToolResponse::Complete(
                CallToolResult::structured_error(serde_json::json!({ "error": error.to_string() })),
            )),
        }
    }
}

struct BoundedReader<R> {
    inner: R,
    maximum_line_bytes: usize,
    current_line_bytes: usize,
}

impl<R> BoundedReader<R> {
    fn new(inner: R, maximum_line_bytes: usize) -> Self {
        Self {
            inner,
            maximum_line_bytes,
            current_line_bytes: 0,
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for BoundedReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut TaskContext<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if buffer.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let remaining = self
            .maximum_line_bytes
            .saturating_sub(self.current_line_bytes)
            .saturating_add(1)
            .min(8192);
        let mut scratch = [0; 8192];
        let mut limited = ReadBuf::new(&mut scratch[..remaining]);
        match Pin::new(&mut self.inner).poll_read(context, &mut limited) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
            Poll::Ready(Ok(())) => {
                let bytes = limited.filled();
                let trailing_line_bytes = bytes
                    .iter()
                    .rposition(|byte| *byte == b'\n')
                    .map_or(self.current_line_bytes + bytes.len(), |index| {
                        bytes.len() - index - 1
                    });
                if trailing_line_bytes > self.maximum_line_bytes {
                    Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("MCP frame exceeds {} bytes", self.maximum_line_bytes),
                    )))
                } else {
                    buffer.put_slice(bytes);
                    self.current_line_bytes = trailing_line_bytes;
                    Poll::Ready(Ok(()))
                }
            }
        }
    }
}

fn tool_name(definition: &CommandDefinition) -> String {
    format!("eps--{}", definition.id.replace('/', "--"))
}

fn annotations(safety: Safety) -> ToolAnnotations {
    ToolAnnotations::new()
        .read_only(matches!(safety, Safety::ReadOnly))
        .destructive(matches!(safety, Safety::DestructiveLocal))
        .open_world(matches!(safety, Safety::RemoteWrite))
}

fn mcp_schema(definition: &CommandDefinition) -> JsonObject {
    let mut schema = definition.schema.as_object().cloned().unwrap_or_default();
    if !matches!(definition.safety, Safety::ReadOnly) {
        let properties = schema
            .entry("properties")
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("registry schemas always have object properties");
        properties.insert(
            "apply".to_owned(),
            serde_json::json!({
                "const": true,
                "description": "Explicitly approve this mutation."
            }),
        );
        let required = schema
            .entry("required")
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .expect("registry schemas always use an array for required fields");
        if !required.iter().any(|item| item == "apply") {
            required.push(Value::String("apply".to_owned()));
        }
    }
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        for property in properties.values_mut() {
            property
                .as_object_mut()
                .expect("registry properties are schema objects")
                .remove("x-episodic-positional");
        }
    }
    schema
}

fn require_apply(definition: &CommandDefinition, arguments: &JsonObject) -> Result<()> {
    if !matches!(definition.safety, Safety::ReadOnly)
        && arguments.get("apply") != Some(&Value::Bool(true))
    {
        bail!("{} requires apply: true", definition.id);
    }
    Ok(())
}

fn cli_arguments(definition: &CommandDefinition, arguments: &JsonObject) -> Result<Vec<String>> {
    let properties = definition
        .schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("registry schemas always have object properties");
    let mut flags = Vec::new();
    let mut positionals = Vec::new();
    for (name, value) in arguments {
        if name == "apply" && matches!(definition.safety, Safety::LocalWrite) {
            continue;
        }
        let Some(schema) = properties.get(name) else {
            bail!("unknown argument {name}");
        };
        let values = string_values(name, value)?;
        if let Some(index) = schema.get("x-episodic-positional").and_then(Value::as_u64) {
            positionals.push((index, values));
        } else if value == &Value::Bool(true) {
            flags.push(format!("--{name}"));
        } else if value != &Value::Bool(false) {
            for value in values {
                flags.push(format!("--{name}={value}"));
            }
        }
    }
    positionals.sort_by_key(|(index, _)| *index);
    if !positionals.is_empty() {
        flags.push("--".to_owned());
        flags.extend(positionals.into_iter().flat_map(|(_, values)| values));
    }
    Ok(flags)
}

fn string_values(name: &str, value: &Value) -> Result<Vec<String>> {
    match value {
        Value::String(value) => Ok(vec![value.clone()]),
        Value::Bool(_) => Ok(Vec::new()),
        Value::Array(values) => values
            .iter()
            .map(|value| match value {
                Value::String(value) => Ok(value.clone()),
                _ => bail!("{name} must contain strings"),
            })
            .collect(),
        _ => bail!("{name} must be a string, boolean, or array of strings"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn bounded_reader_rejects_an_oversized_frame() {
        let (mut writer, reader) = tokio::io::duplex(8);
        writer.write_all(b"abcd").await.unwrap();
        let mut reader = BoundedReader::new(reader, 3);
        let mut destination = Vec::new();

        assert!(reader.read_to_end(&mut destination).await.is_err());
    }

    #[test]
    fn mutating_schemas_require_apply() {
        let registry = Registry::from_clap(&Cli::command());
        let definition = registry
            .commands()
            .iter()
            .find(|definition| definition.id == "episode/set")
            .unwrap();
        let schema = mcp_schema(definition);

        assert_eq!(schema["properties"]["apply"]["const"], true);
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&Value::String("apply".into()))
        );
    }

    #[test]
    fn local_apply_is_an_mcp_gate_not_a_cli_argument() {
        let registry = Registry::from_clap(&Cli::command());
        let definition = registry
            .commands()
            .iter()
            .find(|definition| definition.id == "episode/set")
            .unwrap();
        let arguments =
            serde_json::json!({ "path": "episode", "assignments": ["Title=New"], "apply": true });

        assert_eq!(
            cli_arguments(definition, arguments.as_object().unwrap()).unwrap(),
            ["--", "episode", "Title=New"]
        );
    }
}
