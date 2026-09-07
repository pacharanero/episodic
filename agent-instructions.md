# Agent Instructions

`episodic` is a local-first Rust engine and CLI for episode production workflows. It is not a hosted service, database, video editor, transcription engine, or autonomous publishing agent.

This file is the entry point for AI coding agents. Read it before changing anything.

## Read First

- [README.md](README.md) - project purpose and current setup.
- [spec/README.md](spec/README.md) - specification index and reading order.
- [spec/roadmap.md](spec/roadmap.md) - implementation status and next milestones.
- [../spec/queries.md](../spec/queries.md) - decisions waiting for Marcus.
- `~/code/house-style/AGENTS.md` - cross-project engineering standards.

## Core Invariants

- All durable state remains in ordinary local files. Do not introduce a database or hosted control plane.
- Keep domain models independent of the eventual canonical episode-state serialization. The format decision is deliberately deferred (D1 in `spec/decisions.md`), so nothing may assume Markdown, JSON, YAML, or TOML.
- Generic workflow behaviour belongs in the core; channel identity and policy belong in configuration or channel documentation.
- Read-only commands never mutate local or remote state. Future local writes honour `--dry-run`; remote or destructive actions additionally require `--apply`.
- Standard data goes to stdout. Progress, hints, diagnostics, and errors go to stderr.
- Preserve unknown human-authored content when reading or updating legacy episode metadata.
- A failed external command or invalid generated artefact must never be published as complete state.

## Workflow

- `s/test` - run all tests.
- `s/lint` - run formatting checks and strict Clippy.
- `s/build` - create a release build.

## Before Every Commit

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
reuse lint
```

## Assurance

- Review the resulting diff and test evidence rather than treating agent output as proof.
- Use real but redacted legacy episode shapes as migration fixtures.
- Test remote integrations behind fakes or recorded responses before enabling writes.

## Approval Required

Ask before publishing releases, uploading or publishing episodes, posting to social platforms, deleting or moving episode media, changing secrets, or taking any externally visible action.
