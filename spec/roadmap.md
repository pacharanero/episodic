# Episodic Roadmap

Legend: `[x]` done, `[~]` in progress or partially done, `[ ]` not started. Item codes are stable and must not be renumbered or reused.

## Foundation

- [x] **R1 - Establish the standalone project scaffold.** Create a reusable Rust library and `eps` binary, AGPL-3.0-or-later licensing, REUSE metadata, agent instructions, security policy, scripts, and committed dependency lockfile.
- [x] **R2 - Separate generic and channel documentation.** Generic architecture, state, lifecycle, and artefact contracts live in `spec/`. Following D2, all Everything Digital Health configuration and policy documents have moved out to the EDH workspace under `channels/everything-digital-health/`; this repository keeps only the schema, `channels/example.toml`, and redacted fixtures, so extraction to a standalone project is a move rather than a cleanup.
- [~] **R3 - Implement global CLI conventions.** Text/JSON/YAML output, helpful bare commands, version aliases, tilde expansion, SIGPIPE handling, global channel/config/dry-run options, and basic completions exist. Completion installation and machine-discoverable command schemas remain.
- [~] **R4 - Implement the channel engine.** The complete configuration schema is now typed: generic core (`channel`, `paths`, `llm`, `storage`, `transcription`, `thumbnail`, `description`, `hashtags`) plus typed integration records (`youtube`, `playlists`, `credentials.*`, `recording.*`, `archive.*`) per D4. Unknown top-level sections are rejected, and validation covers channel identity, YouTube privacy, LLM profile/task routing, the `<base>` upload template, and thumbnail dimensions. Schema versioning, path resolution relative to config, and resolving secret references remain.
- [~] **R5 - Implement read-only episode inspection.** The legacy Markdown adapter discovers managed fields and artefacts, validates local upload readiness, checks chapter coverage and thumbnail dimensions, and reports lifecycle inconsistencies. Lossless updates now exist separately in R6. More historical fixtures remain, and inspection still uses a flat field index that cannot distinguish same-named fields in different sections the way the writer does.

## Core Workflow

- [~] **R6 - Implement lossless legacy Markdown updates.** D1 defers the choice of a new canonical format until real `eps` usage shows what it must carry, so `<base>.episode-data.md` stays canonical and the domain model stays serialization-independent.

  Done: `episodic::document` parses a document into lines with their exact terminators and rebuilds it byte-for-byte; `eps episode set` updates managed `- **Label**: value` fields and `eps episode set-draft` replaces a fenced draft block. Content inside code fences is never mistaken for a managed field, an ambiguous label requires `--section` rather than guessing, an unknown label is an error rather than an invented line, a value that is not a placeholder needs `--force`, a repeated update is a no-op, writes are staged beside the target and renamed over it, and the global `--dry-run` prints the same plan an apply would perform. Verified byte-for-byte against all 23 real EDH episode documents through the opt-in `EPISODIC_CORPUS` conformance test.

  Remaining: inserting an absent platform section under the documented rule, updating the `Playlists to Add` checkbox list, and the fenced tag block. Schema versioning and any migration plan wait on the deferred format decision.
- [ ] **R7 - Implement episode creation, listing, validation, and guarded promotion.** Every mutation supports complete dry-run output and atomic writes, reusing the R6 document writer rather than templating over an existing file.
- [ ] **R8 - Port deterministic media commands.** Add audio analysis/processing, extraction/transcription orchestration, frame extraction, thumbnail composition, and external-tool validation. Audio reproduces the working single-mix chain exactly (D8): `highpass -> afftdn -> dynaudnorm -> two-pass loudnorm`.
- [ ] **R9 - Port generated metadata commands.** Add provider-neutral chapters, tags, descriptions, quotes, and social drafts with condensed transcript inputs, bounded retries, schemas, and validation.
- [ ] **R10 - Implement YouTube planning, upload, verification, and reconciliation.** Orchestrate `yutu`, default uploads to unlisted, require `--apply`, and record only verified outcomes. Guest approval is not a machine gate (D5): `--apply` on the public-publish action is the human checkpoint.
- [ ] **R11 - Implement social planning, posting, and verification.** Keep drafting separate from posting and record confirmed destination URLs.
- [ ] **R12 - Implement retention audit and guarded cleanup.** Preserve exact upload sources and quarantine selected candidates before deletion.

## Additional Surfaces

- [ ] **R13 - Establish one command registry and JSON schemas.** Drive CLI discovery, templates, MCP tools, and GUI operations from the same typed definitions.
- [ ] **R14 - Implement `eps mcp`.** Expose read-only and planning operations normally; require explicit apply fields for mutations.
- [ ] **R15 - Implement the local GUI.** Reuse the engine, command plans, safety policy, and filesystem watcher; do not create a second backend or database.
- [ ] **R16 - Add Everything Discourse as the second real channel.** Deferred: work on that channel has not started (D6), and its configuration must not be invented ahead of a real episode. Until then, channel neutrality is defended by the D2 distribution boundary and the D4 core/integration split rather than by a second live channel.

## Distribution

- [ ] **R17 - Add release automation and installers.** Adopt the house-style cargo-dist, auto-tag, changelog, Homebrew, shell completion, and standard target path when the interface is ready to ship.
- [ ] **R18 - Define release readiness.** Complete one EDH episode end-to-end through `eps`, one Everything Discourse episode through configuration only, agent/MCP safety scenarios, and clean-machine doctor/install tests.
