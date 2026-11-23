# Product

<!-- impeccable:product-schema 1 -->
<!-- SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd -->
<!-- SPDX-License-Identifier: CC-BY-SA-4.0 -->

## Platform

web

The browser workbench is a local web surface served by `eps gui`, alongside the Rust CLI and MCP surfaces. It runs in the user's normal browser, not a hosted service or native desktop application (D13).

## Users

Independent episode producers operating their own local channel workspace. They need to see production progress, inspect episode artefacts and review the next production step without assembling separate CLI results and broken long links. This audience and purpose were confirmed during init.

## Product purpose

`episodic` produces and publishes video and podcast episodes from ordinary local files. The workbench brings episode stages, inspection findings, production links and model routing into one place. Success means a producer can understand what exists, what has been checked and what needs doing next, while retaining control of files and publication.

## Positioning

The filesystem is the source of truth. Channel policy is configuration rather than channel-specific engine code. `eps` orchestrates FFmpeg, Whisper, ImageMagick and configured model providers instead of replacing them. CLI, MCP and GUI share the engine; the GUI adds no independent workflow logic. Publication records distinguish confirmed remote outcomes from unverified or unconfirmed work.

## Operating context

One channel workspace is selected explicitly by `--config`, otherwise by `eps.toml` in the current directory. Each GUI server serves one workspace on loopback; a second channel uses a second server. Episode folders, show notes, records and generated artefacts remain readable outside `eps`.

The workflow moves through planning, recording and editing, audio processing, transcription, metadata and thumbnail review, guarded YouTube upload, publication and retention. Producers also use recording tools, file managers, editors, YouTube Studio and platform composers. External links open only on a deliberate click.

## Capabilities and constraints

- The current GUI is a read-only development preview: lifecycle board, episode inspection, artefact presence, recorded YouTube links and configured model routing. Refresh rereads files. It cannot edit, move episodes, generate metadata or publish.
- R33-R35 in `spec/gui.md` distinguish read-and-link work, local command execution and remote actions. Planned capabilities must not be presented as shipped.
- Durable episode state stays in local files, configuration in `eps.toml`. Legacy Markdown records remain canonical while the future serialization decision is deferred (D1). Preserve unknown human-authored content.
- YouTube is the only remote publication destination. Social drafts and human-operated compose links do not authorise posting or scheduling.
- Read-only operations never mutate state. Local writes honour dry-run; remote and destructive operations require explicit apply. GUI execution must use registry commands and the documented plan-then-apply safeguards. Making a video public stays a separate human action.
- Local readiness, file presence and recorded publication state are different claims. Do not imply a media check or remote verification that did not run.
- Show task, profile, provider and model before generation. Channel defaults remain configuration edits; per-run overrides belong to the shared command mechanism (D14).
- No database, hosted control plane, multi-user collaboration, editorial calendar, video editor, autonomous publishing, analytics, SEO or A/B-testing service.
- The page must not read or expose credentials or contact remote services on load. Loopback access uses a per-launch token and Host/Origin checks.

Open decisions remain in `spec/gui.md`: frontend release packaging (G1), job-log location (G2), stronger staleness evidence (G3) and configuration-defined policy milestones (G4). This record does not settle them.

## Brand commitments

The project name is `episodic`; the installed command is `eps`. Use the engine's terms: episode, stage, artefact, edited master, upload master, transcript family, task, profile, plan and apply. Milestones are derived checks, not independently stored episode status. Channel identity and editorial policy belong to external channel configuration, not the generic product.

## Evidence on hand

- `README.md` documents the implemented CLI and reports one real episode produced and uploaded end to end. This is not evidence of second-channel acceptance or a finished GUI.
- `spec/gui.md` records the production incidents motivating the workbench and separates accepted design from implemented scope.
- `spec/decisions.md` records binding product decisions, including the local browser platform and single-workspace boundary.
- `channels/example.toml` and `tests/fixtures/` provide fictional configuration and redacted compatibility examples. Production channel assets, credentials and episode state remain outside this repository.
- `gui/e2e/README.md` documents synthetic browser checks and their limits. Its reflow proxy is not proof of full WCAG conformance.

## Product principles

- Files remain authoritative and usable without the application.
- Report the evidence actually available; distinguish present, checked, confirmed and unknown.
- Keep publication and destructive actions deliberate, reviewable and separate.
- Share workflow behaviour across surfaces and keep channel policy in configuration.
- Show the next production task and the tools or model routing it needs.

## Accessibility & inclusion

The GUI target is WCAG 2.2 AA, including keyboard operation of the board and episode workflow. Views need loading, empty, error and disabled states. Verify rendered desktop and narrow layouts, keyboard-only navigation and 200% zoom before declaring a GUI milestone complete. State actual verification coverage rather than claiming conformance from automated checks alone.
