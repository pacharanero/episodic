# Troubleshooting

Start with the local diagnostic command from the channel workspace:

```bash
eps doctor
```

`eps doctor` reports configuration and local prerequisite metadata. It does not execute external tools, open credential files, refresh tokens, or contact services. A clean report therefore does not prove that a model, credential, or remote destination is usable.

## No configuration is selected

Run `eps` from the directory containing `eps.toml`, or supply an explicit configuration path:

```bash
eps --config /path/to/eps.toml channel validate
```

There is no ancestor-directory, environment-variable, or global configuration fallback. This is deliberate: it prevents production commands from operating on the wrong channel.

## A local production tool is missing

Run `eps doctor` after configuring the relevant section. Install the tool required for the task:

- FFmpeg and ffprobe for audio processing and frame extraction.
- Whisper for transcription.
- ImageMagick for thumbnail composition.
- The configured model CLI for `provider = "cli"` metadata generation.

`eps doctor` checks whether an executable or configured asset is present. It does not verify external-tool versions or contact a model provider.

## A YouTube command cannot authenticate

Check the `[credentials.youtube]` configuration and run `eps doctor`. Each credential must use exactly one `_path` or `_env` field. The token JSON must contain a refresh token; access tokens alone expire and cannot be renewed.

See [YouTube setup](youtube-setup.md) for the expected file model. Never paste client secrets or token contents into issue reports, command output, or committed configuration.

## A command would overwrite local work

Generation and composition commands preserve existing artefacts by default. Review the existing file, then use the command's `--force` option only when replacement is intentional. Use `--dry-run` first when a command supports a local write.

## An upload is unconfirmed

YouTube can return stale data immediately after a successful write. `eps` polls for confirmation and reports work as unconfirmed rather than recording it as complete when the destination has not caught up. Run `eps youtube verify path/to/episode` before retrying; it reconciles the local record with the destination.

## Still blocked

Run the relevant command with `--help` to inspect its arguments and use `--format json` when structured output will help automation. Include the command, non-secret diagnostic output, installed `eps --version`, and minimal reproduction steps in a bug report. Report security issues privately as described in [`SECURITY.md`](../SECURITY.md).
