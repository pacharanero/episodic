# First Episode

This guide follows the local production workflow. It does not invent channel policy: add the relevant tool, metadata, thumbnail, and YouTube sections from [`channels/example.toml`](../channels/example.toml) before running commands that need them.

## Create and record

```bash
eps episode new 2026-09-15 'First episode' --guest 'Example guest'
```

Record or place the source media in the episode workspace according to your channel's local conventions. `episodic` does not record or edit video.

## Process audio and transcript

Run audio processing on the source media, then extract and transcribe its audio track:

```bash
eps audio process path/to/first-episode.mp4
eps transcript extract path/to/first-episode.processed.mp4
eps transcript transcribe path/to/first-episode.wav
```

The exact output names use the episode base name. Processing stages outputs beside their destinations and validates them before treating them as complete. Transcription writes the caption family together: SRT, VTT, text, JSON, and TSV.

## Generate and review publication artefacts

When a configured model provider is available, generate the artefacts that match your channel policy:

```bash
eps metadata chapters path/to/first-episode.srt --episode path/to/episode
eps metadata tags path/to/first-episode.srt --episode path/to/episode
eps metadata description --episode path/to/episode --transcript path/to/first-episode.srt
eps metadata quote path/to/first-episode.srt --episode path/to/episode
eps thumbnail frames path/to/first-episode.processed.mp4
eps thumbnail compose path/to/chosen-frame.jpg 'Thumbnail caption'
```

Generated artefacts must satisfy their configured contract before they are written. Existing artefacts are not regenerated unless you explicitly use `--force`, which protects local edits.

Inspect upload readiness before any remote operation:

```bash
eps episode validate path/to/episode
```

Fix each reported missing or inconsistent artefact before proceeding. Use `--dry-run` with write commands to inspect their plan without writing files.

## Plan and upload

Upload planning is read-only and does not contact YouTube:

```bash
eps youtube plan-upload path/to/episode
```

After reviewing the plan, upload explicitly:

```bash
eps youtube upload path/to/episode --apply
```

Uploads default to the channel privacy setting or `unlisted`. Making a video public is a separate explicit operation. See [YouTube setup](youtube-setup.md) before using remote commands, then use `eps youtube verify path/to/episode` to reconcile the local record with YouTube.
