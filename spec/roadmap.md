# Episodic Roadmap

Legend: `[x]` done, `[~]` in progress or partially done, `[ ]` not started. Item codes are stable and must not be renumbered or reused.

## Foundation

- [x] **R1 - Establish the standalone project scaffold.** Create a reusable Rust library and `eps` binary, AGPL-3.0-or-later licensing, REUSE metadata, agent instructions, security policy, scripts, and committed dependency lockfile.
- [x] **R2 - Separate generic and channel documentation.** Generic architecture, state, lifecycle, and artefact contracts live in `spec/`. This is now the standalone source repository (D9), containing only the schema, `channels/example.toml`, and redacted fixtures. Production configuration, channel documents, assets, credentials, and state remain in external workspaces; compatibility tooling cutover is tracked there.
- [~] **R3 - Implement global CLI conventions.** Text/JSON/YAML output, helpful bare commands, version aliases, tilde expansion, SIGPIPE handling, global config/dry-run options, and basic completions exist. `--channel` was removed in D9. Completion installation and machine-discoverable command schemas remain.
- [~] **R4 - Implement the channel engine.** The configuration schema is typed with strict parsing and semantic validation on normal loads. Selection is explicit `--config`, then PWD `eps.toml` only, with no environment/global/ancestor fallback or nested-layout heuristic (D9). Relative assets resolve against the normalized absolute config parent. `channel list` reports the one selected workspace. Tests cover implicit and explicit selection, invalid and missing configs, no ancestor traversal, and config-relative episode paths. Schema versioning and broader secret-reference support remain.
- [~] **R5 - Implement read-only episode inspection.** The legacy Markdown adapter discovers managed fields and artefacts, validates local upload readiness, checks chapter coverage and thumbnail dimensions, and reports lifecycle inconsistencies. Lossless updates now exist separately in R6, and inspection shares the writer's fence-aware scan and can address a field as `Section / Label`. More historical fixtures remain.

## Core Workflow

- [x] **R6 - Implement lossless legacy Markdown updates.** D1 defers the choice of a new canonical format until real `eps` usage shows what it must carry, so `<base>.episode-data.md` stays canonical and the domain model stays serialization-independent.

  `episodic::document` parses a document into lines with their exact terminators and rebuilds it byte-for-byte. `eps episode set` updates managed `- **Label**: value` fields, `set-draft` replaces a fenced draft block, `set-tags` replaces the tag list in either the fenced or bare shape and refreshes the derived character count, `mark-playlist` ticks one playlist entry, and `add-platform` inserts a missing social section.

  Content inside code fences is never mistaken for a managed field, checkbox updates are confined to the playlist section so the manual studio steps cannot be ticked by accident, playlists are matched by ID rather than by display name, an ambiguous label requires `--section` rather than guessing, an unknown label or playlist is an error rather than an invented line, a value that is not a placeholder needs `--force`, repeated updates are no-ops, writes are staged beside the target and renamed over it, and the global `--dry-run` prints the same plan an apply would perform. Readers and writers share one fence-aware scan, so they cannot disagree about what a field is.

  Both document shapes are fixtures, and all 23 real EDH documents round-trip byte-for-byte through the opt-in `EPISODIC_CORPUS` conformance test. The variation found across the real archive is recorded in the parent workspace `spec/episode-metadata-contract.md`.

  Schema versioning and any migration plan wait on the deferred format decision.

- [x] **R7 - Implement episode creation, listing, validation, and guarded promotion.** `eps episode new` creates a planning folder with show notes and a metadata skeleton built entirely from channel configuration: playlists, archive destinations, recording links, YouTube defaults, and host all come from TOML, so a second channel needs no code. The folder is staged in a temporary directory and renamed into place, leaving nothing behind on failure.

  `eps episode list` groups episodes by stage, recognises the `YYYY.MM.DD-slug` convention, descends into year subdirectories for the back catalogue, and reports which episodes have no metadata file.

  `eps episode promote` moves an episode between stages, guarded: publishing without a recorded Video ID, moving backwards through the lifecycle, and leaving `not-for-publishing` all require `--force` and are reported as overridden. A destination that already exists is refused outright, because the same episode in two stage directories is invalid state. `--dry-run` moves nothing.
- [x] **R8 - Port deterministic media commands.** `eps transcript extract` produces the standardised mono track, `eps audio process` applies the channel's two-pass loudness chain, `eps audio analyse` measures without changing anything, and `eps transcript transcribe` produces the srt/vtt/txt/json/tsv family.

  Artefact names come from one place. `paths::episode_base` derives the `<base>` every durable artefact shares and strips the `.processed` infix that only the upload master carries, so a pipeline running on the processed master still emits the names the artefact contract declares and `episode validate` can find them (F2). A full-pipeline test asserts the order and the resulting names.

  Outputs are staged beside their destination, probed, and only then renamed, so a tool that fails, or that exits successfully having written something unreadable, leaves nothing behind that a later step would treat as finished work. The transcript family is all-or-nothing: a truncated SRT would silently become the input to chapters and captions. Repeat runs are skips, video streams are copied rather than re-encoded, and `--dry-run` prints the real command line.

  Transcripts are clamped while still staged, so Whisper's trailing-silence hallucinations are not published as captions (F3). Cues starting at or after the media duration are dropped on arithmetic alone; a trailing run of repeated short phrases is dropped only once `volumedetect` confirms the span is silent, because the same text over real audio is someone saying it twice. All five formats are corrected together and the kept bytes are preserved, so SRT numbering, the `WEBVTT` header, and Whisper's per-segment JSON fields survive. Removals are reported rather than silent, and a measurement that cannot be taken publishes the family unclamped with the limitation stated rather than discarding a transcript that cost minutes of GPU time.

  The chain is configuration, not constants: `[audio]` carries the high-pass, denoise floor, loudness targets, bitrate, and sample rates, with the podcast defaults built in so the section may be omitted entirely.

  `eps thumbnail frames` extracts evenly spaced interior frames, skipping the first and last twentieth so intro and outro slates are not offered as candidates. Extraction is per-frame idempotent: a frame that already exists is never regenerated, because it may be the one already chosen or hand-cropped.

  `eps thumbnail compose` overlays the caption and emits both the upload PNG and an editable SVG companion referencing the frame by relative path. Both must exist for the pair to count as done, so a half-finished pair is retried rather than mistaken for success, and both are staged before either is published. Because choosing a frame and a caption is iterative, `--force` recomposes over a finished pair (F4). Canvas size, font, point size, fill, stroke, adaptive frame contrast, uppercasing, and quoting all come from `[thumbnail]`. Adaptive contrast selects black or white text from the frame luminance and uses the opposite colour for the outline. Verified pixel-identical to the shell implementation on the same input when adaptive contrast is disabled.

  The parent workspace's `s/process` now composes these focused commands with `eps metadata chapters|tags|quote`, using JSON output to carry each canonical path into the next step. Its compatibility fixture proves the six-step order and the clean `<base>` naming when the source is `<base>.processed.mp4`.

- [~] **R9 - Port generated metadata commands.** `eps metadata chapters|tags|quote|description|social` generate through a provider-neutral adapter, and `eps metadata check` validates an existing artefact without calling a model.

  Generation, validation, and retry are one operation: nothing is written until an attempt satisfies its task contract, and a run that cannot produce a valid answer in three attempts fails loudly rather than writing the last bad one. Rejected attempts are reported even when a later one succeeds, so a model that is drifting is visible. An empty response is a failure, not an artefact. An existing artefact is never regenerated without `--force`, which protects human edits.

  Transcripts are condensed into fixed-length blocks before prompting, so prompt size tracks episode duration rather than Whisper's cue count. The chapter coverage check compares the last chapter against the transcript end, which is what catches a model that only saw part of the episode: shape alone cannot.

  Task contracts are configuration. `[metadata]` carries chapter counts and coverage, tag counts and any required final tag, description headings, quote counts, and per-platform social limits, so the EDH-specific values that were hardcoded in the shell validators are now channel policy.

  Remaining: `provider = "cli"` is the only implemented transport. Native provider routing follows the same path as D8.

- [x] **R10 - Implement YouTube planning, upload, verification, and reconciliation.** `eps youtube plan-upload|upload|verify|publish|captions|thumbnail|playlist` talk to the Data API through `google-youtube3` (D8). Authentication reuses the workspace's existing refresh token through the authorised-user flow, so no browser and no `yutu` are involved; a token with no refresh token is refused with an explanation rather than an obscure failure.

  Command logic depends on a `Destination` trait rather than on the API client, so planning, applying, verifying, and recording are all tested against an in-memory fake. The API implementation is the only code that talks to Google, and the async runtime is contained inside it: `eps` is a one-shot process with nothing to overlap, so the rest of the engine stays synchronous.

  Uploads use the resumable endpoint, default to unlisted, and require `--apply`; making a video public is a separate command. The opaque resumable-session URL is kept in an owner-readable hidden episode file, reused automatically by the next invocation after interruption, and removed after successful completion. The video ID is written to the record the moment the upload returns, before anything else is attempted, because everything after that can be retried and an unrecorded ID cannot be recovered. Nothing else is recorded until the destination confirms it: a playlist only where YouTube reports the video in it, captions only where YouTube reports tracks, and the thumbnail, which cannot be read back, labelled unverified.

  Confirmation allows for eventual consistency. The Data API can report the pre-write state immediately after a successful write, so confirmation is polled six times over about a minute rather than attempted once (F1). There are three outcomes, not two: confirmed, unconfirmed, and failed. A write that returns an error is a failure; a read that has not caught up is not, and neither is a read that errors, because under eventual consistency absence is never proof. Unconfirmed work records nothing, exits zero, and tells the operator to re-check with `verify` before retrying. A playlist that was successfully asked for gets an unticked row in the record at the point of the call, so the repair path always has a line to tick and an unconfirmed playlist is still recoverable (F5); only confirmation ticks it.

  Verified against the live API: reading a published episode returned its privacy, caption count, and playlist membership, and reconciliation correctly reported an episode whose record says unlisted while YouTube says public.

  Standalone caption, thumbnail, and playlist commands repair an already uploaded episode. They plan without contacting YouTube unless `--apply` is present, verify captions and playlist membership before recording them, and label thumbnail API acceptance as unverified because the API cannot read the thumbnail back.

  External channel upload wrappers can invoke this native adapter directly and apply their own playlist policy. Their migration and real-episode acceptance are tracked in the channel workspace, not inferred from a parent checkout. `yutu` is not required by the engine.

- [ ] **R12 - Implement retention audit and guarded cleanup.** Preserve exact upload sources and quarantine selected candidates before deletion.

## Additional Surfaces

- [ ] **R13 - Establish one command registry and JSON schemas.** Drive CLI discovery, templates, MCP tools, and GUI operations from the same typed definitions.
- [ ] **R14 - Implement `eps mcp`.** Expose read-only and planning operations normally; require explicit apply fields for mutations.
- [ ] **R16 - Add Everything Discourse as the second real channel.** Deferred: work on that channel has not started (D6), and its configuration must not be invented ahead of a real episode. Until then, channel neutrality is defended by the D2 distribution boundary and the D4 core/integration split rather than by a second live channel.

## Distribution

- [ ] **R17 - Add release automation and installers.** Adopt the house-style cargo-dist, auto-tag, changelog, Homebrew, shell completion, and standard target path when the interface is ready to ship.
- [ ] **R18 - Define release readiness.** Complete one EDH episode end-to-end through `eps`, one Everything Discourse episode through configuration only, agent/MCP safety scenarios, and clean-machine doctor/install tests.
- [x] **R23 - Implement standalone doctor diagnostics.** `eps doctor` reports canonical selected config/root, episode directory, runtime metadata task templates and routing, prompt support files, font, production tool lookup (including Whisper and ImageMagick fallbacks), CLI LLM executables, and YouTube credential presence/Unix permission bits. Text/JSON/YAML reports are emitted before nonzero exit for prerequisite errors; optional setup and unused unimplemented providers are warnings. Invalid explicit tool overrides do not fall back. Isolated temporary-workspace fixtures cover missing setup, overrides, routing, permissions, config selection, and non-execution/non-disclosure. No subprocesses, credential reads, downloads, or remote calls: versions are not checked, model availability and credential validity remain unverified. Effective access/ACLs, parent-directory permissions, and asset contents are outside this metadata-only check; release installation acceptance remains R18.
