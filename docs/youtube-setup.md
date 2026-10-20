# YouTube Setup

`eps` publishes directly through the YouTube Data API using the native `google-youtube3` adapter. It does not use a browser, a third-party CLI, or a hosted intermediary during upload.

## Prepare the Google credentials

Before using YouTube commands, prepare an authorised-user OAuth setup for the channel:

1. Create or select a Google Cloud project for the channel and enable the YouTube Data API.
2. Create an OAuth client and save its client-secret JSON file outside version control.
3. Complete the OAuth consent flow with tooling that produces an authorised-user token JSON containing a refresh token.
4. Keep both files in a private channel workspace location, not in this repository or an episode directory.

`eps` does not begin an interactive consent flow. It reuses the existing client secret and authorised-user token, refreshes access without opening a browser, and rejects a token that has no refresh token.

## Reference credential files from configuration

Add one path or one environment-variable name for each file. Relative paths resolve from the directory containing `eps.toml`.

```toml
[credentials.youtube]
client_secret_path = "credentials/client-secret.json"
token_path = "credentials/youtube-token.json"
```

Alternatively, environment variables may hold the paths to ignored local files:

```toml
[credentials.youtube]
client_secret_env = "YOUTUBE_CLIENT_SECRET_PATH"
token_env = "YOUTUBE_TOKEN_PATH"
```

Do not set both the `_path` and `_env` form for the same file. The environment variables contain file paths, never credential values. `eps doctor` reports that a reference exists but never reads the variable value or the credential file.

## Configure the destination

Add the channel's YouTube policy to `eps.toml` as needed:

```toml
[youtube]
channel_id = "UCexampleexampleexampl"
handle = "@ExampleChannel"
default_privacy = "unlisted"
```

Use [`channels/example.toml`](../channels/example.toml) for supported category, language, licence, playlist, and credential fields.

## Publish safely

Start with a read-only plan:

```bash
eps youtube plan-upload path/to/episode
```

Remote writes require `--apply`:

```bash
eps youtube upload path/to/episode --apply
```

`eps` records the video ID as soon as an upload returns, then records captions and playlists only after YouTube confirms them. Thumbnail updates are labelled unverified because the API cannot read them back. Verify a published episode at any time:

```bash
eps youtube verify path/to/episode
```

Uploads default to `unlisted` when neither a command option nor channel policy sets privacy. Publishing a video publicly is a separate command and still requires explicit application.
