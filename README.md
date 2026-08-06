# episodic

`episodic` is a local-first toolkit for planning, producing, publishing, and maintaining video and podcast episodes. Its installed command is `eps`.

The filesystem is the source of truth. Episode state, channel policy, generated metadata, and durable publication records remain ordinary local files; `episodic` does not introduce a database or hosted control plane.

## Status

Early. `eps` can validate channel TOML, inspect episode folders, report upload-readiness and lifecycle inconsistencies, and make targeted lossless edits to the episode record. It does not yet process media, generate metadata, or talk to any remote service.

Edits change only the labelled fields and draft blocks they are asked to change: no template rebuild, no reordering, no reflowing, and no normalising of whitespace or line endings. A field already holding a real value is never replaced without `--force`, and a repeated update is a no-op rather than a rewrite.

## Quick Start

```bash
cargo run -- version
cargo run -- --config channels/example.toml channel validate
cargo run -- episode inspect ../episodes/in-progress/2026.07.29-making-the-nhs-numbers-python-package
cargo run -- --format json episode inspect ../episodes/in-progress/2026.07.29-making-the-nhs-numbers-python-package

# Preview an edit, then make it.
cargo run -- --dry-run episode set <episode> 'Video ID=dQw4w9WgXcQ'
cargo run -- episode set <episode> 'Post URL=https://bsky.app/...' --section Bluesky
cargo run -- episode set-draft <episode> Bluesky < draft.txt
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
