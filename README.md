# episodic

`episodic` is a local-first toolkit for planning, producing, publishing, and maintaining video and podcast episodes. Its installed command is `eps`.

The filesystem is the source of truth. Episode state, channel policy, generated metadata, and durable publication records remain ordinary local files; `episodic` does not introduce a database or hosted control plane.

## Status

The project is at the first read-only implementation milestone. It can validate channel TOML, inspect current Everything Digital Health episode folders, and report upload-readiness and lifecycle inconsistencies without changing files or remote services.

## Quick Start

```bash
cargo run -- version
cargo run -- --config channels/example.toml channel validate
cargo run -- episode inspect ../episodes/in-progress/2026.07.29-making-the-nhs-numbers-python-package
cargo run -- --format json episode inspect ../episodes/in-progress/2026.07.29-making-the-nhs-numbers-python-package
```

Use `eps --help`, `eps channel`, or `eps episode` to discover the current surface. Every command supports text, JSON, and YAML output through the global `--format` option.

## Scope

`episodic` orchestrates proven tools such as FFmpeg, Whisper, ImageMagick, `yutu`, and configured language-model providers. It does not replace a video editor, recording platform, transcription engine, or remote platform API.

The generic tooling specification lives in [`spec/`](spec/README.md). Channel-specific policy belongs under [`channels/`](channels/README.md) or in the external channel workspace. The source documentation map records how existing Everything Digital Health documentation was classified rather than copying stale operational prose into the core.

## Development

```bash
s/test
s/lint
s/build
```

The relevant engineering standards are in `~/code/house-style/AGENTS.md`. See [`agent-instructions.md`](agent-instructions.md) before making changes.

## Licensing

Source code is licensed under the [GNU Affero General Public License v3.0 or later](LICENSE). Documentation and specification content is licensed under CC-BY-SA-4.0. This project is deliberately not available for proprietary subsumption; commercial proprietary use requires a separate licence.
