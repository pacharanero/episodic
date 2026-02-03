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

1. **Set up Google credentials** using Application Default Credentials (ADC):

   ```bash
   # Option A: Use a service account JSON file
   export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json

   # Option B: Use your own Google account (requires gcloud CLI)
   gcloud auth application-default login
   ```

   The service account (or user) needs access to:
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

- The Rust CLI includes the interactive flow and formatting, and implements
  Google Drive/Docs/Calendar operations using Application Default Credentials.
- Configuration is loaded from `edh-live.env` in the repository root.
