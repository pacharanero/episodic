# episodic

`episodic` is a local-first toolkit for planning, producing, publishing, and maintaining video and podcast episodes. Its installed command is `eps`.

The filesystem is the source of truth. Episode state, channel policy, generated metadata, and durable publication records remain ordinary local files; `episodic` does not introduce a database or hosted control plane.

## Status

Early, but a real episode can now be produced with it. `eps` creates and lists episodes, moves them between lifecycle stages, processes audio, transcribes, generates chapters, tags, descriptions, quotes and social drafts, extracts frames, composes thumbnails, and makes targeted lossless edits to the episode record. Its native YouTube adapter plans, uploads, verifies, publishes, and repairs captions, thumbnails, and playlist membership through `google-youtube3`; it does not require `yutu`. Remote writes require `--apply`, and making a video public is a separate action. Nothing is recorded that the destination has not confirmed, and because the Data API is eventually consistent, confirmation is polled for about a minute: work that has not appeared by then is reported as unconfirmed rather than failed, records nothing, and exits zero.

New episodes are built entirely from channel configuration, so adding a channel is a TOML file rather than a code change.

Edits change only what they are asked to change: no template rebuild, no reordering, no reflowing, and no normalising of whitespace or line endings. A field already holding a real value is never replaced without `--force`, a repeated update is a no-op rather than a rewrite, and every document shape in the real archive round-trips byte-for-byte.

## Quick Start

Build from this standalone source checkout with `s/build`; the binary is `target/release/eps`. Release installers remain future work; `eps doctor` checks local workspace prerequisites.

Each channel has its own external workspace with an `eps.toml` file, episodes, prompts, and other assets. Use [`channels/example.toml`](channels/example.toml) as a schema reference and include only the sections you need. A minimal workspace configuration is:

```toml
[channel]
id = "my-channel"
name = "My Channel"
timezone = "Europe/London"

[paths]
episodes = "episodes"
```

With `eps` available on your PATH, run these commands from that workspace:

```bash
eps version
eps doctor
eps channel validate
eps channel list
eps episode list
eps --format json episode inspect <episode>

# Preview an edit, then make it.
eps --dry-run episode set <episode> 'Video ID=dQw4w9WgXcQ'
eps episode set <episode> 'Post URL=https://bsky.app/...' --section Bluesky
eps episode set-draft <episode> Bluesky < draft.txt
eps episode set-tags <episode> < tags.txt
eps episode mark-playlist <episode> PLxxxx --note 'Added 2026-08-07T09:00:00Z'
eps episode add-platform <episode> Mastodon

# Media and metadata.
eps audio process <recording>
eps transcript extract <recording> && eps transcript transcribe <wav>
eps metadata chapters <srt> --output <base>.chapters.txt
eps thumbnail frames <recording> --count 8
eps thumbnail compose <frame> 'Caption text'

# Lifecycle.
eps episode new 2026-09-15 'A new episode' --guest 'Example Guest'
eps episode list --stage in-progress
eps --dry-run episode promote <episode> edited
```

Configuration-dependent commands select explicit `--config <path>` first, otherwise only `eps.toml` in PWD. From elsewhere, use `eps --config /path/to/workspace/eps.toml episode list` (relative selectors are PWD-relative too). There is no ancestor search, environment selector, global fallback, or `--channel` selection. Missing, malformed, or invalid selected configuration fails rather than falling back. Relative config assets resolve against the normalized absolute config parent; explicit episode and media arguments remain PWD-relative. `channel list` returns just the selected workspace channel, not a registry or directory scan.

Use `eps --help`, `eps channel`, or `eps episode` to discover the current surface. Every command supports text, JSON, and YAML output through the global `--format` option.

## Local Diagnostics

Run `eps doctor`, or `eps --config /path/to/workspace/eps.toml --format json doctor` (also `--format yaml`). It emits a report on stdout before exiting: 0 means no prerequisite errors, 1 means errors were found, with a summary on stderr. Warnings alone do not fail. `--dry-run` is accepted and changes nothing: doctor is always read-only.

Checks cover the canonical selected config and workspace root, episode directory, FFmpeg/ffprobe, ImageMagick (`magick` then `convert`), Whisper (PATH then `venv/bin/whisper`), configured CLI LLM executables, prompt templates and support files, thumbnail font, and native YouTube credential file presence and Unix permission bits. Tool lookup shares production's `EPISODIC_<NAME>_BIN` overrides; an invalid explicit override never silently falls back. Relative assets retain production's config-parent resolution; tool overrides and CLI executable paths retain production's PWD-relative semantics.

When metadata or LLM configuration is present, templates include the runtime tasks `chapters`, `tags`, `thumbnail-quote`, and `youtube-description`, plus configured LLM task routes and social-policy tasks. Default-profile routing is reported too. Missing required tools or configured files are errors. Absent optional LLM, thumbnail, and YouTube configuration is a warning; unimplemented LLM providers are errors when routed to a task and warnings when unused. A non-Whisper transcription backend is an error because production currently invokes Whisper regardless of that setting.

Doctor never executes tools, reads credential values, refreshes tokens, calls remote services, or downloads models. Versions are explicitly not checked, and model availability remains unverified. File checks use metadata only: content validity, prompt rendering, font validity, effective access/ACLs, and parent-directory permissions are not verified. Group/other credential permission bits produce a warning recommending owner-only permissions. Invalid configuration is reported without echoing TOML source lines that could contain misplaced secrets.

## Scope

`episodic` orchestrates proven tools such as FFmpeg, Whisper, ImageMagick, and configured language-model CLIs, and calls the YouTube API natively. It does not replace a video editor, recording platform, transcription engine, or remote platform API. Native LLM transports remain future work; currently model generation uses `provider = "cli"`.

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
