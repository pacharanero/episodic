# Channels

Channel configuration contains identity and policy; the `episodic` engine contains workflow behaviour. Adding a channel must not require source-code branches.

This repository holds the schema, [`example.toml`](example.toml), and redacted conformance fixtures only. Production channel configuration, editorial policy, credentials, and episode state live in each channel's own workspace and are never committed here, so `episodic` can be published as a general-purpose tool (D2). The Everything Digital Health configuration and channel documents now live in the EDH workspace under `channels/everything-digital-health/`.

## Schema Shape

A configuration is a small generic core plus typed integration records (D4). Only `[channel]` is required, and no integration section is needed to load or validate a config.

| Core | Purpose |
|---|---|
| `[channel]` | Stable ID, display name, host, timezone, subject, voice. |
| `[paths]` | Workspace locations, resolved relative to the workspace root. |
| `[llm]` | Provider-neutral profiles and per-task routing. |
| `[storage]` | Retention intervals and the canonical upload-source template. |
| `[transcription]` | Backend, model, language, device. |
| `[thumbnail]` | Dimensions and text-composition policy. |
| `[description]`, `[hashtags]` | Reusable publication text. |

| Integration | Purpose |
|---|---|
| `[youtube]`, `[playlists]` | Destination identity, default privacy, playlist policy. |
| `[credentials.*]` | Pointers to where a credential lives. |
| `[recording.*]` | Non-secret joining details, keyed by tool. |
| `[archive.*]` | Long-term destinations, keyed by target name. |

A channel that publishes nowhere but YouTube carries no archive or social sections. Unknown top-level sections are rejected rather than ignored, so a typo fails validation instead of silently disabling policy.

## Adaptive Thumbnail Contrast

Set `adaptive_contrast = true` under `[thumbnail]` to choose black or white caption text from the selected frame's luminance and use the opposite colour for its outline. This overrides configured `fill` and `stroke` values during composition. Leave it disabled to retain fixed brand colours.

## What Does Not Belong In Channel Configuration

- OAuth tokens, API keys, passwords, client secrets, or private guest data.
- Generic command behaviour or adapter implementation.
- Per-episode outcomes such as video IDs, selected playlists, publication dates, or social URLs.
- Generated remote caches.

## Second Channel

Everything Discourse is the intended second real channel for proving that `episodic` is not EDH-specific. Work on it has not started (D6), and its configuration must not be invented ahead of a real episode.
