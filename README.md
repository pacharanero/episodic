# `edh`

`edh` is a command-line tool to automate common setup tasks for a new
Everything Digital Health episode using Google Drive, Docs and Calendar. It is
intended to be composable so that individual steps can be reused from other
tools.

## Rust CLI

This repository now includes a Rust implementation of the CLI.

### Install

```bash
cargo install --path .
```

This installs the `edh` binary to `~/.cargo/bin/`.

### Run

```bash
edh new
```

### Setup

1. **Set up Google OAuth credentials:**
   1. Go to [Google Cloud Console](https://console.cloud.google.com/) and create a new project (or use an existing one)
   2. Enable the required APIs:
      - [Google Drive API](https://console.cloud.google.com/apis/library/drive.googleapis.com)
      - [Google Docs API](https://console.cloud.google.com/apis/library/docs.googleapis.com)
      - [Google Calendar API](https://console.cloud.google.com/apis/library/calendar-json.googleapis.com)
   3. Configure the OAuth consent screen:
      - Go to [OAuth consent screen](https://console.cloud.google.com/apis/credentials/consent)
      - Choose "External" user type
      - Fill in the app name (e.g., "EDH CLI") and your email
      - Add your email as a test user
      - Save and continue through the screens
   4. Create OAuth credentials:
      - Go to [Credentials](https://console.cloud.google.com/apis/credentials)
      - Click **"+ CREATE CREDENTIALS"** → **"OAuth client ID"**
      - Application type: **Desktop app**
      - Name: "EDH CLI"
      - Click **Create**
      - Click **"DOWNLOAD JSON"**
   5. Save the credentials file:
      ```bash
      mkdir -p .secrets
      mv ~/Downloads/client_secret_*.json .secrets/
      ```

2. **Configure environment variables** by copying the template and filling in your URLs:

   ```bash
   cp edh-template.env edh-live.env
   # Edit edh-live.env with your Google Doc/Folder URLs
   ```

   You can paste URLs directly from your browser — the CLI extracts the resource ID automatically.

3. **Run the CLI**:

   ```bash
   # During development (without installing)
   cargo run -- new

   # Or after installing
   edh new
   ```

   On first run, a browser window will open for you to authorize with your Google account.
   Tokens are cached at `~/.edh/token_cache.json` so you only need to authorize once.

   You can also pass arguments directly:

   ```bash
   edh new "My Episode Title" "2026-02-15"
   ```

### Notes

- Authentication uses OAuth2 with your Google account.
- Configuration is loaded from `edh-live.env` in the repository root.
