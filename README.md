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

### Testing in Development

1. **Set up Google authentication** using one of the following methods:

   **Option A: Service Account (recommended for automation)**

   Place your service account JSON file in the `.secrets/` directory:

   ```bash
   mkdir -p .secrets
   cp /path/to/service-account.json .secrets/
   ```

   The CLI will automatically use any `.json` file found in `.secrets/`.

   Alternatively, set the path explicitly:

   ```bash
   export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json
   ```

   **Option B: gcloud CLI (for development)**

   Requires [Google Cloud SDK](https://cloud.google.com/sdk/docs/install) to be installed.

   ```bash
   gcloud auth application-default login
   ```

   **Option C: Interactive OAuth (for personal use)**

   If no service account or gcloud credentials are found, the CLI will automatically
   start an interactive browser-based OAuth flow. To use this:
   1. Create OAuth 2.0 credentials in [Google Cloud Console](https://console.cloud.google.com/apis/credentials)
      - Choose "Desktop app" as the application type
   2. Download the JSON file and save it as one of:
      - `./client_secret.json` (in the project directory)
      - `./edh-client-secret.json`
      - `~/.edh/client_secret.json`
   3. Run `edh new` — a browser window will open for authorization
   4. Tokens are cached at `~/.edh/token_cache.json` so you only authorize once

   The account needs access to:
   - Google Drive (create folders, copy files)
   - Google Docs (read/write document content)
   - Google Calendar (create events)

2. **Configure environment variables** by copying the template and filling in your IDs:

   ```bash
   cp edh-template.env edh-live.env
   # Edit edh-live.env with your Google Doc/Folder/Calendar IDs
   ```

3. **Run the CLI**:

   ```bash
   # During development (without installing)
   cargo run -- new

   # Or after installing
   edh new
   ```

   You can also pass arguments directly to skip prompts:

   ```bash
   edh new "My Episode Title" "2026-02-15"
   ```

### Notes

- The Rust CLI supports multiple authentication methods: service account, gcloud ADC,
  or interactive browser-based OAuth.
- Configuration is loaded from `edh-live.env` in the repository root.
