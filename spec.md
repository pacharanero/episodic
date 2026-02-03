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

Google access uses Application Default Credentials (ADC).

- Preferred: set `GOOGLE_APPLICATION_CREDENTIALS` to a service-account JSON
  with access to Drive, Docs and Calendar.
- Alternatively: `gcloud auth application-default login` (user creds)
- Fallback: interactive browser-based OAuth flow (requires `client_secret.json`)

Configuration values are loaded from `edh-live.env` in the repo root. URLs can be
pasted directly from the browser — the CLI extracts the resource ID automatically.

### Defaults and conventions (current behavior)

- Title input is accepted as-entered (no normalization or Title Case conversion).
- Kebab-case is derived from the entered title:
  - trim leading/trailing whitespace
  - replace spaces/underscores with `-`
  - remove non-alphanumeric characters (except `-`)
  - collapse repeated `-`
  - lowercase
- Proposed Date input must match `YYYY-MM-DD`, then must parse as a real calendar date.
- Episode Guide document edits:
  - Replace the first Heading 1 text with the Episode Title.
  - Insert the following text immediately after the Heading 1:
    - `Proposed Date: YYYY-MM-DD`
    - `Episode Folder: <folder URL>`
- Calendar event:
  - Use calendar ID from `EDH_CALENDAR_ID` (default to `primary` if unset).
  - Create a single all-day event on Proposed Date (end date = next day).
  - Event title = Episode Title.
- Planned Episodes update (START_HERE_EDH_GDOC_URL):
  - Treated as a Google Doc (not a Sheet).
  - Append a line to the end of the doc:
    - `YYYY-MM-DD - Episode Title - <folder URL>`

### Preproduction (`edh new`)

- [ ] **PROMPT**: "Episode Title (Title Case):" (required; no automatic Title Case conversion)
- [ ] **PROMPT**: "Proposed Date (ISO 8601 YYYY-MM-DD):" (required; regex + real-date validation)
- [ ] **CONVERT**: Convert the date to `YYYY.MM.DD` for folder naming only.
- [ ] **DISPLAY**: Echo:
  - Episode Title
  - `Date: YYYY-MM-DD -> YYYY.MM.DD`
  - `Folder name: YYYY.MM.DD-kebab-case-title`
- [ ] **CONFIRM**: Create a new folder in Google Drive base folder (`EDH_IN_PLANNING_GDRIVE_FOLDER_URL`) named `YYYY.MM.DD-kebab-case-title`.
- [ ] **CONFIRM**: Copy the Episode Guide Template into that folder and rename the copied file to the Episode Title. Attempt to open the copied file in the user's default browser; if that fails, print the URL.
- [ ] **CONFIRM**: "Make Episode Guide document changes?" If yes:
  - replace the first Heading 1 in the document to the Episode Title
  - insert `Proposed Date: YYYY-MM-DD`
  - insert `Episode Folder: <folder URL>`
  - If no folder URL exists (because the folder wasn't created), skip and warn.
- [ ] **CONFIRM**: "Add an all-day event to Google Calendar with the episode title/date?"
- [ ] **CONFIRM**: "Append the episode to the Planned Episodes doc?"
  - Append a line to the end of the doc: `YYYY-MM-DD - Episode Title - <folder URL>`
  - Attempt to open the doc in the user's default browser; if that fails, print the URL.
  - If no folder URL exists, skip and warn.

Notes:

- The confirm prompts are Yes/No only (no explicit Skip/Exit options today).

### Editing (`edh edit`)

(PLANNING ONLY - DO NOT IMPLEMENT YET)

### Post-production (`edh post`)

(PLANNING ONLY - DO NOT IMPLEMENT YET)

- Create the thumbnail image from the recording.
- Upload the recording to YouTube with proper title, description, tags, and thumbnail. Defaults are available for all of the settings that must be chose for a new YouTube upload (assuming they can be set via the YouTube API)
- Add Youtube link to the episode guide document.

### Publish (`edh publish`)

(PLANNING ONLY - DO NOT IMPLEMENT YET)

Copy workflows from Zapier and n8n to here.

- [ ] Publish the episode with a short synopsis and link, truncated to Bluesky limits.
- [ ] Publish the episode with a longer synopsis and link, on LinkedIn.
- [ ] Archive the data for the episode in the Baw Medical Discourse forum. Append new post to Topic https://bawmedical.co.uk/t/everything-digital-health-archive-everythingdigitalhealth/1197
