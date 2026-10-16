# Architecture

## Decision

Build `episodic` as a distributable Rust project whose installed executable is `eps`. The binary is a packaging and consistency boundary around focused, composable commands; it is not a monolithic "make my episode" workflow.

The filesystem remains the source of truth. The core is reusable by the CLI, a future local web GUI, and a future MCP server. Those surfaces must call the same engine and typed contracts rather than reimplementing workflow logic.

## Goals

- Support multiple channels through configuration without source-code branches.
- Keep episode state, channel configuration, plans, generated artefacts, and publication records in ordinary local files.
- Give humans, scripts, agents, and GUIs stable text and structured output.
- Orchestrate proven external tools instead of reimplementing media processing, transcription, image composition, or platform APIs.
- Make remote and destructive actions inspectable, explicitly approved, and verifiable.
- Preserve compatibility wrappers until each replacement has passed real acceptance fixtures.

## Non-Goals

- A hosted service, SaaS product, central database, or remote control plane.
- A replacement for a video editor, OBS, recording platform, FFmpeg, Whisper, ImageMagick, or a language-model provider. YouTube operations use a native adapter rather than `yutu` orchestration.
- Autonomous publication or social posting.
- Separate, divergent command contracts for CLI, GUI, and MCP.

## Command Shape

```text
eps [--config <path>] [--format text|json|yaml] [--dry-run]

eps channel list|show|validate
eps episode new|inspect|list|validate|promote
eps audio analyse|process
eps transcript extract|transcribe
eps metadata chapters|tags|description|social
eps thumbnail frames|compose
eps youtube plan-upload|upload|captions|thumbnail|playlist|publish|verify|reconcile
eps cleanup plan|apply
eps storage audit
eps commands
eps <command> --schema|--template
eps doctor
eps mcp
eps completions
eps version
```

Every command writes result data to stdout and progress, diagnostics, and errors to stderr. Text is the human default; JSON is the stable automation contract; YAML is a structured human-readable view. Bare `eps` and bare command families print useful help and exit successfully.

`storage audit` is read-only. `cleanup plan` is read-only, and `cleanup apply` additionally requires `--apply` before it moves selected paths into episode-local quarantine. `eps commands`, `<command> --schema`, and `<command> --template` expose the command inventory and input contracts without loading a workspace. `eps mcp` serves the same registry over stdio; a mutating tool call must state `apply: true`, and remote or destructive commands retain their native apply gate. `eps youtube description` reads and compares the live description with the local formatted draft before a guarded update. A separate reconcile command remains future work. Doctor provides local metadata-only prerequisite checks (R23). See the roadmap and live `eps --help` for status.

## Configuration Selection

One external channel workspace is selected per invocation: explicit `--config <path>` first, otherwise `eps.toml` in PWD only (D9). Relative selectors are PWD-relative. No ancestor traversal, global fallback, environment selector, channel registry, or directory-layout heuristic is permitted. Selection never falls through on a missing or invalid file. Normal loads parse and validate; diagnostic validation shares parsing and issue collection without calling the normal load recursively.

Relative configuration assets resolve against the normalized absolute config parent, irrespective of its directory name. Explicit episode/media/output arguments remain PWD-relative. `channel list` reports a one-element list for the selected valid channel and fails on load errors; it does not scan for other channels.

## Engine Boundary

The Rust library owns domain types, configuration loading, workspace discovery, legacy-state adapters, validation, plans, safety policy, artefact naming, and adapter interfaces. `main.rs` only performs startup handling and delegates to the library.

The initial implementation is a single package with a library and `eps` binary. A separate core crate is warranted only if independent publication, versioning, or MSRV requirements emerge.

## Distribution Boundary

`episodic` is a standalone source repository for a general-purpose tool, so it carries no configuration, policy, or state belonging to a specific channel (D2, D9). It contains the schema, a documented example configuration, and redacted conformance fixtures; production channel configuration and editorial policy live in each channel's own external workspace. Compatibility scripts migrate separately and iteratively; the engine must not infer their former nested layout.

## Adapters

| Adapter | Responsibility |
|---|---|
| Media | Invoke FFmpeg and FFprobe for analysis, audio processing, extraction, and validation. The first audio implementation reproduces the currently working single-mix chain (D8); multi-track profiles are not part of the port. |
| Transcription | Invoke Whisper or a configured equivalent and validate the transcript family. |
| LLM | Route named tasks to configured providers, apply bounded retries, and validate outputs before publication. |
| Image | Invoke ImageMagick for frame and thumbnail operations. |
| YouTube | Call the Data API through `google-youtube3` (D8), with resumable upload and direct OAuth refresh. |

Adapters must expose the command and version used, redact secrets, and return useful error context. Empty or malformed output is failure, not a completed artefact.

## Safety Model

- Read-only commands never mutate local or remote state.
- Every local mutation supports `--dry-run` and emits the complete proposed plan.
- Remote writes and destructive local actions additionally require `--apply`.
- Upload defaults to unlisted. Making a video public is a distinct command.
- `eps metadata social` produces validated drafts; separate platform tools or MCP servers own social posting and verification.
- Every remote action has an inspect or verify counterpart.
- Cleanup first moves candidates into episode-local quarantine; permanent deletion is a separate future policy.
- Credentials and sensitive values are never printed or stored in channel configuration.

## Future Surfaces

`eps mcp` consumes the same command registry, plans, schemas, and safety gates as the CLI. It launches the current executable with JSON output for each tool call, keeping protocol frames isolated while preserving the CLI parser and typed registry route. It may not bypass `--apply` semantics or write episode files directly behind the engine's back. Separate filesystem/frontmatter visualization tools may read lifecycle state without becoming part of the engine.
