# Channels

Channel configuration contains identity and policy; the `episodic` engine contains workflow behaviour. Adding a channel must not require source-code branches.

This standalone repository holds the schema, [`example.toml`](example.toml), and redacted conformance fixtures only. Production channel configuration, editorial policy, credentials, and episode state live in each channel's own external workspace and are never committed here (D2, D9).

## Workspace Selection

Place `eps.toml` at the root of a channel workspace and invoke `eps` there. From anywhere else, supply `--config <path>`; relative selectors are relative to PWD. The explicit selector wins over PWD `eps.toml`. No ancestor directories, environment variables (including `EPS_CONFIG`), global locations, or legacy `channels/<id>/channel.toml` layouts are searched. A missing or invalid selected file fails without fallback.

All relative configuration assets, including `paths.episodes`, prompt files, thumbnail fonts, and credential references, resolve against the normalized absolute config parent. Normalization follows filesystem symlinks, so a symlinked config uses the real config's parent, and `..` is resolved with filesystem semantics. Prompts are read from `prompts/` beside the config. The default episode directory is `episodes/` beside the config. Directory names have no special meaning: a config explicitly selected under `channels/` still resolves against its own parent. CLI episode, media, and output arguments remain PWD-relative.

`eps channel show` reports the selected config; `eps channel list` returns a one-element list for that same workspace and fails if it cannot load it. `eps channel validate` emits a diagnostic report, including all existing semantic validation issues. Normal loads also reject semantic errors, unknown keys and sections, and malformed TOML. Migration is iterative: move or explicitly select existing configs and adjust their relative paths; lossless legacy episode editing remains supported. No automatic config or episode migration is performed.

## Schema Shape

A configuration is a small generic core plus typed integration records (D4). Only `[channel]` is required, and no integration section is needed to load or validate a config. `schema_version = 1` is recommended for new workspaces; omitted legacy configurations are interpreted as version 1. An unsupported version fails clearly rather than being guessed or migrated automatically.

| Core | Purpose |
|---|---|
| `[channel]` | Stable ID, display name, host, timezone, subject, voice. |
| `[paths]` | Workspace locations, resolved relative to the config parent. |
| `[llm]` | Provider-neutral profiles and per-task routing. |
| `[storage]` | Retention intervals and the canonical upload-source template. |
| `[transcription]` | Backend, model, language, device. |
| `[thumbnail]` | Dimensions and text-composition policy. |
| `[description]`, `[hashtags]` | Reusable publication text. |

| Integration | Purpose |
|---|---|
| `[youtube]`, `[playlists]` | Destination identity, default privacy, playlist policy. |
| `[credentials.*]` | Pointers to where a credential file lives. |
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

For the native YouTube adapter, set exactly one of `client_secret_path` or `client_secret_env`, and exactly one of `token_path` or `token_env`, under `[credentials.youtube]`. A `*_path` is config-relative unless absolute. A `*_env` names an environment variable containing that file path. Environment variables therefore provide indirection to ignored local files; they do not carry the credential value itself. `eps doctor` reports configured environment references but never reads them or their target files.

## Second Channel

Everything Discourse is the intended second real channel for proving that `episodic` is not EDH-specific. Work on it has not started (D6), and its configuration must not be invented ahead of a real episode.
