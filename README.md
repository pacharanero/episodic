# episodic

`episodic` is a local-first toolkit for planning, producing, publishing, and maintaining video and podcast episodes. Its installed command is `eps`.

The filesystem is the source of truth. Episode state, channel policy, generated metadata, and durable publication records remain ordinary local files; `episodic` does not introduce a database or hosted control plane.

## Status

Early, but a real episode can now be produced with it. `eps` creates and lists episodes, moves them between lifecycle stages, processes audio, transcribes, generates chapters, tags, descriptions, quotes and social drafts, extracts frames, composes thumbnails, and makes targeted lossless edits to the episode record. It does not yet talk to any remote service: uploading and publishing are still the responsibility of the channel workspace's own tooling.

New episodes are built entirely from channel configuration, so adding a channel is a TOML file rather than a code change.

Edits change only what they are asked to change: no template rebuild, no reordering, no reflowing, and no normalising of whitespace or line endings. A field already holding a real value is never replaced without `--force`, a repeated update is a no-op rather than a rewrite, and every document shape in the real archive round-trips byte-for-byte.

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
cargo run -- episode set-tags <episode> < tags.txt
cargo run -- episode mark-playlist <episode> PLxxxx --note 'Added 2026-08-07T09:00:00Z'
cargo run -- episode add-platform <episode> Mastodon

# Media and metadata.
cargo run -- audio process <recording>
cargo run -- transcript extract <recording> && cargo run -- transcript transcribe <wav>
cargo run -- metadata chapters <srt> --output <base>.chapters.txt
cargo run -- thumbnail frames <recording> --count 8
cargo run -- thumbnail compose <frame> 'Caption text'

# Lifecycle.
cargo run -- episode new 2026-09-15 'Rural informatics' --guest 'Dr Alan Hassey'
cargo run -- episode list --stage in-progress
cargo run -- --dry-run episode promote <episode> edited
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
