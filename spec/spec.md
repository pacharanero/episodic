# EVERYTHING DIGITAL HEALTH SETUP CLI (Rust)

`edh` automates common setup tasks for a new Everything Digital Health episode
using Google Drive, Google Docs and Google Calendar.

## UX + libraries

The CLI should feel visually impressive and polished.

- `clap` for commands/subcommands/help
- `inquire` for interactive prompts + validation
- `console` for consistent color/formatting
- `indicatif` for spinners/progress

## Auth

Google access uses OAuth2 with a desktop app user-consent flow.

- Place a `client_secret*.json` file (from Google Cloud Console) in `.secrets/`,
  the project root, or `~/.edh/`.
- On first run, a browser opens for Google authorization.
- Tokens are cached at `~/.edh/token_cache.json`.

Configuration values are loaded from `edh-live.env` in the repo root. URLs can
be pasted directly from the browser; the CLI extracts the resource ID
automatically.

### Defaults and conventions (current behavior)

- Title input is accepted as-entered (no normalization or Title Case conversion).
- Kebab-case is derived from the entered title:
  - trim leading/trailing whitespace
  - replace spaces/underscores with `-`
  - remove non-alphanumeric characters (except `-`)
  - collapse repeated `-`
  - lowercase
- Proposed Date input must match `YYYY-MM-DD`, then must parse as a real
  calendar date.
- Episode Guide document edits:
  - Replace the first Heading 1 text with the Episode Title.
  - Insert the following text immediately after the Heading 1:
    - `Proposed Date: YYYY-MM-DD`
    - `Episode Folder: <folder URL>`
- Calendar event:
  - Use calendar ID from `EDH_CALENDAR_ID` (default to `primary` if unset).
  - Create a single all-day event on Proposed Date (end date = next day).
  - Event title = Episode Title.
- Planned Episodes update (`START_HERE_EDH_GDOC_URL`):
  - Treated as a Google Doc (not a Sheet).
  - Append a line to the end of the doc:
    - `YYYY-MM-DD - Episode Title - <folder URL>`

### Preproduction (`edh new`)

This command can be used with arguments for automation, or without for full
interactive prompting.

- [x] **PROMPT**: "Episode Title (Title Case):" (required; no automatic Title
      Case conversion)
- [x] **PROMPT**: "Proposed Date (YYYY-MM-DD):" (required; parse validation;
      prompt defaults to today's date)
- [x] **CONVERT**: Convert the date to `YYYY.MM.DD` for folder naming only.
- [x] **DISPLAY**: Echo:
  - Episode Title
  - `Date: YYYY-MM-DD -> YYYY.MM.DD`
  - `Folder name: YYYY.MM.DD-kebab-case-title`
- [x] **CONFIRM**: Create a new folder in Google Drive base folder
      (`EDH_IN_PLANNING_GDRIVE_FOLDER_URL`) named `YYYY.MM.DD-kebab-case-title`.
- [x] **CONFIRM**: Copy the Episode Guide Template into that folder and rename
      the copied file to the Episode Title. Attempt to open the copied file in the
      user's default browser; if that fails, print the URL.
- [x] **CONFIRM**: "Make Episode Guide document changes?" If yes:
  - replace the first Heading 1 in the document to the Episode Title
  - insert `Proposed Date: YYYY-MM-DD`
  - insert `Episode Folder: <folder URL>`
  - If no folder URL exists (because the folder wasn't created), skip and warn.
- [x] **CONFIRM**: "Add an all-day event to Google Calendar with the episode
      title/date?"
- [x] **CONFIRM**: "Append the episode to the Planned Episodes doc?"
  - Append a line to the end of the doc:
    `YYYY-MM-DD - Episode Title - <folder URL>`
  - Attempt to open the doc in the user's default browser; if that fails, print
    the URL.
  - If no folder URL exists, skip and warn.

Notes:

- Confirm prompts are Yes/No and default to Yes.

---

### Editing (`edh edit`)

Current implementation status: not implemented yet.

- [ ] Create a new editing project in the selected video editing tool.
- [ ] Move episode folder from in-planning to in-progress folder.
- [ ] Update episode guide document with actual recording date.
- [ ] Normalize audio volume for all channels to -16 LUFS.
- [ ] Trim leading and trailing silence from the recording.

---

### Post-production (`edh post`)

Current implementation status: not implemented yet.

#### Transcription

- [ ] Transcribe the recording using local Parakeet (API key from env var).

#### Show notes & links

- [ ] Create show notes text from the transcription using LLM + prompt (API key
      from env var).
- [ ] Add show notes to the episode guide document under the Show notes & Links
      heading.

#### Thumbnail

- [ ] Create thumbnail text from transcription using LLM + prompt (API key from
      env var), then confirm with the user.
- [ ] Create thumbnail image from the recording using Canva API (API key from
      env var):
  - create a new copy of the Canva image template
  - apply the thumbnail text

#### YouTube upload

- [ ] Create tag list from show notes text using LLM + prompt (API key from env
      var).
- [ ] Upload recording to YouTube with title, description, tags, and thumbnail.
- [ ] Add YouTube link to the episode guide document under heading YouTube.
- [ ] Add ALL uploads to the following Playlists:
  - Everything Digital Health Audio-Only Podcast Version
  - EVERYTHING Everything Digital Health - BINGE WATCH
- [ ] Add to other playlists according to the content

---

### Publish (`edh publish`)

Current implementation status: not implemented yet.

- [ ] Copy/translate workflows from Zapier and n8n into this CLI.
- [ ] **Bluesky**: publish episode with a short synopsis and link, truncated to
      Bluesky limits.
- [ ] **LinkedIn**: publish episode with a longer synopsis and link.
- [ ] **Baw Medical Archive**: append a new archive post to topic
      https://bawmedical.co.uk/t/everything-digital-health-archive-everythingdigitalhealth/1197

## Documentation

- Any new changes that affect user-visible behavior should be reflected in
  `README.md` and this spec file.

## TODO

- appearance - colours
- applicability to other podcasts; how to generalize
- error handling (e.g. Google API errors)
- logging
- tests
- check n8n and Zapier workflows, decommission once shifted to here
- cancel Zapier subscription after edh is fully functional
