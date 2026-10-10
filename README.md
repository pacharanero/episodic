# episodic

**Plan, produce, and publish video and podcast episodes from the command line.** One tool for the whole lifecycle of a YouTube channel: planning an episode, mastering its audio, transcribing it, writing its chapters, tags and description, building its thumbnail, uploading it, and keeping the record of what was published honest.

[![CI](https://github.com/pacharanero/episodic/actions/workflows/ci.yml/badge.svg)](https://github.com/pacharanero/episodic/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/episodic-cli.svg)](https://crates.io/crates/episodic-cli)
[![Licence: AGPL-3.0-or-later](https://img.shields.io/badge/licence-AGPL--3.0--or--later-blue.svg)](LICENSE)

The installed command is `eps`.

```console
$ eps episode new 2026-09-15 'EHRs and cognitive load' --guest 'Dr Example'
Created episodes/in-planning/2026.09.15-ehrs-and-cognitive-load
  2026.09.15-ehrs-and-cognitive-load.md (160 bytes)
  2026.09.15-ehrs-and-cognitive-load.episode-data.md (2270 bytes)

$ eps audio process 2026.09.15-ehrs-and-cognitive-load.mp4
Wrote ./2026.09.15-ehrs-and-cognitive-load.processed.mp4
  measured -19.0 LUFS, normalised to -16.0 LUFS

$ eps transcript extract 2026.09.15-ehrs-and-cognitive-load.processed.mp4
Wrote ./2026.09.15-ehrs-and-cognitive-load.wav

$ eps transcript transcribe 2026.09.15-ehrs-and-cognitive-load.wav
Transcribing with turbo on cpu; this takes a while.
Transcript: removed 10 of 233 cues: 10 repeated cue(s) from 20:55 over silence (-91.0 dBFS peak)
Wrote ./2026.09.15-ehrs-and-cognitive-load.srt, ./2026.09.15-ehrs-and-cognitive-load.vtt, ...

$ eps episode validate .
Episode: EHRs and cognitive load
Stage: in-planning
Ready for upload: no
Artefacts:
- captions: present (./2026.09.15-ehrs-and-cognitive-load.srt)
- chapters: missing (./2026.09.15-ehrs-and-cognitive-load.chapters.txt)
- tags: missing (./2026.09.15-ehrs-and-cognitive-load.tags.txt)
  ... and the rest of the artefact list
Findings: none
Next action: Record the episode
```

Then `eps metadata chapters|tags|description`, `eps thumbnail frames|compose`, and `eps youtube upload --apply`, which records only what YouTube confirms:

```console
$ eps youtube upload 2026.09.15-ehrs-and-cognitive-load --apply
Video ID: dQw4w9WgXcQ
  confirmed: upload
  confirmed: captions (1 track(s))
  confirmed: thumbnail (unverified)
  confirmed: privacy unlisted
  recorded: Video ID
  recorded: YouTube URL
  recorded: Upload Date
  recorded: Captions
```

## Install

```bash
cargo install episodic-cli             # from crates.io; installs `eps`
cargo binstall episodic-cli            # prebuilt binary, requires cargo-binstall
```

Or without a Rust toolchain, on macOS and Linux:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/pacharanero/episodic/releases/latest/download/episodic-cli-installer.sh | sh
```

On Windows, PowerShell:

```powershell
irm https://github.com/pacharanero/episodic/releases/latest/download/episodic-cli-installer.ps1 | iex
```

An `.msi` is attached to each release, and prebuilt binaries are published for macOS (Apple silicon and Intel), Linux (x86-64 and ARM64), and Windows (x86-64).

`eps` orchestrates tools you already have rather than reimplementing them, so install what you intend to use: **FFmpeg** for audio and frames, **Whisper** for transcription, **ImageMagick** for thumbnails, and a **language-model CLI** for generated metadata. Run `eps doctor` to see what is present and what is missing.

## Quick start

A channel is a directory with an `eps.toml` in it. The smallest useful one:

```toml
[channel]
id = "my-channel"
name = "My Channel"
timezone = "Europe/London"

[paths]
episodes = "episodes"
```

Then, from that directory:

```bash
eps doctor                              # what is installed, what is missing
eps channel validate                    # is the configuration sound
eps episode new 2026-09-15 'First episode'
eps episode list
```

Every command takes `--format json` for scripting, and `--dry-run` to see exactly what would happen without doing it.

## What it is

A **local-first** toolkit. The filesystem is the source of truth: episode state, channel policy, generated metadata, and publication records are all ordinary files in your own directories. There is no database, no hosted control plane, and no account to create. Delete `eps` and your episodes are still there, still readable, still yours.

Adding a channel is a TOML file, not a code change. Editorial policy - chapter counts, tag limits, description headings, loudness targets, thumbnail fonts, playlist IDs - is configuration, so a second channel needs no new code.

**What it is not:** a video editor, a recording platform, a transcription engine, a replacement for FFmpeg or Whisper or ImageMagick, or an autonomous publishing agent. It does not post to social platforms. It drives the tools that do the work, and it is careful about what it records as true.

## The episode lifecycle

### Planning

`eps episode new` creates the episode folder, show notes, and a metadata skeleton built entirely from channel configuration - playlists, archive destinations, recording links, YouTube defaults, host. `eps episode list` groups everything by lifecycle stage, understands the `YYYY.MM.DD-slug` convention, and descends into year subdirectories for a back catalogue. `eps episode promote` moves an episode between stages, and refuses to publish an episode with no recorded video ID, or to move backwards, without `--force`.

### Pre-production and audio

`eps audio analyse` measures loudness and changes nothing. `eps audio process` applies the channel's two-pass loudness chain - high-pass, denoise, dynamic normalisation, then linear `loudnorm` using measurements from the first pass - and copies the video stream rather than re-encoding it, so mastering never costs a generation of picture quality.

### Post-production

| Step | Command |
|---|---|
| Transcribe | `eps transcript extract`, then `eps transcript transcribe` for the srt/vtt/txt/json/tsv family |
| Chapters | `eps metadata chapters` |
| Tags | `eps metadata tags` |
| Description | `eps metadata description` |
| Thumbnail quotes | `eps metadata quote` |
| Social drafts | `eps metadata social` |
| Frames | `eps thumbnail frames` |
| Thumbnail | `eps thumbnail compose` |

Generation, validation, and retry are one operation. Nothing is written until an attempt satisfies its task contract, and a run that cannot produce a valid answer in three attempts fails loudly rather than writing the last bad one. Rejected attempts are reported even when a later one succeeds, so a model that is drifting is visible. An existing artefact is never regenerated without `--force`, which protects your edits.

Transcripts are clamped to the media before they are published, because Whisper hallucinates over trailing silence: cues starting after the end of the file are dropped, and a trailing run of repeated short phrases is dropped once the audio under it measures silent. Chapter coverage is checked against the end of the transcript, which is what catches a model that only saw part of the episode.

### Publication

`eps youtube plan-upload` shows what an upload would do and contacts nothing. `eps youtube upload --apply` does it, through the YouTube Data API via `google-youtube3` - no browser, no third-party CLI. `verify` compares the record against the destination, and `captions`, `thumbnail` and `playlist` repair an already-uploaded episode one piece at a time.

Uploads use the resumable endpoint, so an interrupted large upload resumes rather than restarting. **Uploads default to unlisted**; making a video public is a separate command.

## Safety

The things that stop a tool like this from ruining an afternoon:

- **Read-only commands never write.** Local writes honour `--dry-run`, and the plan it prints is what an apply performs. Remote and destructive actions additionally require `--apply`.
- **Nothing is recorded that was not confirmed.** The video ID is written the instant the upload returns, because everything after that can be retried and an unrecorded ID cannot be recovered. Captions are recorded only where YouTube reports tracks, a playlist only where YouTube reports the video in it, and the thumbnail - which the API cannot read back - is labelled unverified.
- **Eventual consistency is expected, not mistaken for failure.** Confirmation is polled for about a minute. Work that has not appeared by then is reported as unconfirmed, records nothing, and exits zero.
- **Edits change only what they are asked to change.** No template rebuild, no reordering, no reflowing, no normalising of whitespace or line endings. A field holding a real value is never replaced without `--force`, a repeated update is a no-op rather than a rewrite, and every document shape in the real archive round-trips byte-for-byte.
- **A failed tool publishes nothing.** Outputs are staged beside their destination, probed, and only then renamed. A tool that exits successfully having written something unreadable leaves nothing behind that a later step would treat as finished work.
- **Standard data goes to stdout**, progress and diagnostics to stderr, so piping is safe.

## Configuration

[`channels/example.toml`](channels/example.toml) is a complete, fictional, commented example exercising every section `eps` understands. Only `[channel]` is required; copy the sections you actually need. See [`channels/README.md`](channels/README.md) for the schema and how asset paths resolve.

Configuration is selected by explicit `--config <path>` first, otherwise `eps.toml` in the current directory, and nothing else - no ancestor search, no environment variable, no global fallback. This is deliberately narrower than most tools: with several channels on one machine, the failure mode worth preventing is silently operating on the wrong one.

## Diagnostics

`eps doctor` reports what is configured and what is installed: the selected config and workspace root, episode directory, FFmpeg and ffprobe, ImageMagick, Whisper, configured model CLIs, prompt templates, thumbnail font, and YouTube credential presence and permissions. Errors exit non-zero; absent optional setup is a warning.

It is deliberately conservative. It never executes a tool, reads a credential, refreshes a token, or contacts a service, so it does not check versions and cannot confirm that a model is available or a credential still valid. It tells you what it did not check.

## Status

Early, but real. One full episode has been produced and uploaded with it end to end. The command surface above exists and is tested; what remains is listed in [`spec/roadmap.md`](spec/roadmap.md), and faults found in real use are tracked with reproduction steps in [`spec/defects.md`](spec/defects.md).

Known gaps worth knowing before you adopt: generated metadata currently routes through a configured CLI (`provider = "cli"`) rather than a native API transport; retention audit and cleanup are unimplemented; and the MCP server is designed but not built.

## Development

```bash
s/test     # the whole test suite
s/lint     # formatting check and strict Clippy
s/build    # release binary
```

The design specification lives in [`spec/`](spec/README.md): [architecture](spec/architecture.md), [filesystem state](spec/filesystem-state.md), [episode lifecycle](spec/episode-lifecycle.md), the [artefact contract](spec/artefact-contract.md), and the [resolved decisions](spec/decisions.md) with the reasoning behind them. Read [`agent-instructions.md`](agent-instructions.md) before making changes.

Contributions are welcome. Report a security vulnerability privately as described in [`SECURITY.md`](SECURITY.md), not in a public issue.

Releases are one command, `s/version++ [patch|minor|major]`: it bumps the version, regenerates [`CHANGELOG.md`](CHANGELOG.md), and lands the result on `main`, after which CI tags it and builds and publishes the release.

## Licensing

Code is licensed under the [GNU Affero General Public License v3.0 or later](LICENSE); documentation and specification content under CC-BY-SA-4.0. Copyright Dr Marcus Baw and Koloki Ltd.

This project is deliberately not available for proprietary subsumption. Commercial proprietary use requires a separate licence.
