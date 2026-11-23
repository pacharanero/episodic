# GUI

`eps gui` serves a local web workbench for one channel workspace. It shows every episode's stage and progress as the filesystem records it, gathers the links and tools each step needs in one consistent place, shows which models each generation step will use, and runs `eps` commands through the same registry handlers as the CLI and MCP surfaces. It adds no workflow logic of its own.

Status: draft for agreement (R32). Nothing here is implemented yet.

## Why

Evidence from producing one real episode end to end (17-18 November 2026) is recorded under R32 in [roadmap.md](roadmap.md). In short:

- Long pre-filled links (calendar templates, social compose intents) broke when handed over in an agent chat. A generated local HTML page with labelled buttons worked well.
- Most post-production review was side-by-side checking that needed separate CLI calls: chapters against the transcript, contract limits, frame candidates, uncertain caption terms, and the upload plan before `--apply`.
- A lifecycle move broke an editing project's media links, the render went silent, and nothing flagged it before a GPU transcription run (R37, F9).
- Two GPU jobs from different sessions collided and one failed with an out-of-memory error.

## Decisions

- **Local web, served by `eps` (D13).** The frontend runs in the user's normal browser, where external links behave as they do anywhere else. This is a deliberate exception to the house-style Tauri default; the frontend still follows the house-style stack (React, TypeScript, Mantine, Vite) and [ui.md](https://github.com/pacharanero/house-style/blob/main/ui.md) so it could be wrapped in Tauri later without rewriting it.
- **One workspace per server (D15).** `eps gui` selects configuration exactly as every other command does (D9): `--config <path>`, otherwise `eps.toml` in PWD. A second channel runs a second server.
- **Model routing is always visible; overrides are per run (D14).** The channel default stays in `eps.toml`.
- **Delivery in three milestones**: read and link (R33), local steps (R34), remote steps (R35). Each milestone is usable on its own.

## Product Boundary

The GUI must not:

- hold state of its own beyond view preferences and the job log (see [Jobs](#jobs)). Episode state stays in the episode folder; configuration stays in `eps.toml`.
- write episode files, configuration, or remote state except through registry commands.
- post to social platforms or forums. It may build compose links for a human to click, as `~/code/socials` does, and record a post URL through `episode add-platform`.
- gate on review state that the engine does not gate on (D5).
- contact a remote service on page load. Remote reads such as `youtube verify` run only when the user asks, and their result is shown with the time it was taken.

## Ubiquitous Language

The GUI uses the engine's terms unchanged: episode, stage, artefact, edited master, upload master, transcript family, task, profile, plan, apply. It adds one term:

- **Milestone**: a named check, derived from the files in an episode folder, that says how far an episode has progressed within its stage. Milestones are computed on every read and never stored, for the same reason stages are folders: there is no status field to disagree with the filesystem.

## Stages And Milestones

The board shows the four stages from `src/lifecycle.rs`: `in-planning`, `in-progress`, `published`, and `not-for-publishing`. Within a stage, an episode's progress is its list of milestones. Each milestone has one of these states:

| State | Meaning |
|---|---|
| Not started | The artefacts it checks do not exist. |
| Present | The artefacts exist but have not been checked. |
| Passing | The artefacts exist and pass their contract (`metadata check`, `episode validate`, FFprobe). |
| Failing | A check ran and failed. The failure text is shown. |
| Possibly stale | An artefact is older than the input it was derived from, for example chapters older than the transcript, or an upload master older than the edited master. |
| Not applicable | The channel does not use this destination. |

The channel-neutral milestone set, in production order:

1. **Planned**: canonical episode record and show notes exist; episode date and title set.
2. **Recorded**: at least one source media file exists in the folder besides the canonical artefacts.
3. **Edited master**: `<base>.mp4` exists, FFprobe reads it, and it has audio for its whole length (R37).
4. **Upload master**: `<base>.processed.mp4` exists and is not older than the edited master.
5. **Transcript family**: all five transcript formats exist, cues are ordered and end near the media duration, and no unresolved repeated-cue warning remains.
6. **Metadata**: chapters, tags, description, and thumbnail quote candidates each exist and pass `metadata check`; chapters cover the transcript. Each is its own row.
7. **Thumbnail**: frame candidates exist; the final thumbnail exists and matches channel dimensions.
8. **Upload ready**: `episode validate` reports ready for upload.
9. **Uploaded**: a video ID is recorded; captions, thumbnail, and each selected playlist are separate rows, each recorded only after verification.
10. **Public**: recorded privacy is public and the folder is in `published`. A public remote item in another stage is shown as a reconciliation warning, as [episode-lifecycle.md](episode-lifecycle.md) requires.
11. **Publicised**: one row per platform record added through `episode add-platform`. Channel configuration decides which platforms are expected.

Milestones that depend on channel policy (calendar invitations, guest review, archive posts) come from configuration, not from the engine's built-in list. Until the configuration schema can describe them, they are not shown rather than hard-coded.

The next action for an episode is the first milestone that is not passing. The board shows it on every episode; the episode view offers it as the primary action.

## Views

The design follows [ui.md](https://github.com/pacharanero/house-style/blob/main/ui.md): an operational tool that opens straight into the board, quiet and dense, using borders and spacing rather than decoration.

### Board

The default view. One column per stage, with `not-for-publishing` collapsed. Each episode is one compact card showing the date, title, guest, a milestone strip (one small mark per milestone, with a text label available on focus and hover), and the next action as text. Cards are ordered by episode date. The `published` column shows recent episodes and has a filter, because it holds the whole back catalogue. Planned episodes with no canonical record (which `episode list` already reports) appear with a "No episode record" marker rather than being hidden.

### Episode

One page per episode, reached from the board.

- **Header**: title, date, stage, guest, the folder path with a copy button, and a `Reveal in file manager` button.
- **Milestones**: the full list down the side, each row showing its state and its artefacts. Selecting a row shows its detail.
- **Detail**: the artefacts behind the selected milestone with size, modification time and check result; the command that produces or checks them, with its safety class; and, for generated artefacts, a preview. Previews are text for transcripts, chapters, tags and descriptions, image grids for frame candidates and the thumbnail, and an audio waveform with silent spans marked for media.
- **Links and tools**: a panel that is always in the same place on every episode page. See [Links And Tools](#links-and-tools).

### Channel

The selected workspace and its configuration: identity, paths, the model routing table (see [Model Routing](#model-routing)), transcription settings, playlists, and the latest `doctor` result. Read-only in R33.

### Jobs

Running and recent commands across the workspace, with their stderr progress, exit status, outputs, and a link to the episode. See [Jobs](#jobs).

## Links And Tools

Every link the user needs for a step is generated by the engine and shown in the episode's links panel, grouped by when it is used. Links are ordinary anchors that open in a new browser tab; nothing opens automatically.

- **Recording**: guest, host and director links from the channel's recording configuration.
- **Episode files**: reveal the folder; open the show notes or canonical record in the system's default application. These are explicit, user-initiated actions (see [Local Actions](#local-actions)).
- **YouTube**: watch URL, short URL, and the YouTube Studio edit page, once a video ID is recorded.
- **Publicity**: compose links for each configured platform, built from a draft the user can edit in place, with a live character count against the platform limit. Posting stays a human click in the platform's own composer. A field beside each platform records the resulting post URL through `episode add-platform`.
- **Channel destinations**: standing forum topics and archive pages named in configuration.

Link generation is a read-only engine function, so the same links are available from the CLI (for example `eps episode links <episode>`, part of R33) and the MCP surface. The GUI never assembles a URL itself.

## Model Routing

Before any generation step runs, the GUI shows which task, profile, provider and model it will use, for example "Chapters: profile `terra`, provider `cli`, model `anthropic/claude-sonnet-5-5`". The same information appears in the Channel view as a table of every task.

- **Per-run override (D14).** Where a channel defines more than one profile, a generation step can be run once with a different profile. This needs a `--profile <name>` option on the `metadata` commands (R36), so the CLI and MCP surfaces get the same capability. The override is recorded in the job log, not in configuration.
- **Changing the default** stays an `eps.toml` edit. The Channel view shows the relevant section and its path; it does not write configuration in R33-R35.
- **Transcription** is shown the same way (backend, model, language, device), because it is the other model choice that costs real time.
- **Availability** is reported as `doctor` reports it: configured, not verified, unless the user runs a check.

## Jobs

A job is one registry command run from the GUI. The server runs each job as a child `eps` process with JSON output, exactly as `eps mcp` does, and streams its stderr progress to the page.

- One job at a time per episode, and one GPU-class job (transcription) at a time per workspace. A second request queues and says what it is waiting for.
- Before a GPU-class job starts, the server reports other processes using the GPU where the platform can list them, because a job from outside the GUI can still collide.
- A job's command line, start and end times, exit status, outputs, chosen profile, and stderr are kept as a job log. Job logs are reproducible operational records, not canonical state; their location is open question G2.
- Cancelling a job stops the child process. Because generators stage output and rename only on success, a cancelled job leaves no partial artefact.

## Safety

The registry already labels every command `ReadOnly`, `LocalWrite`, `RemoteWrite` or `DestructiveLocal`. The GUI shows that label on every action button and follows it:

- **ReadOnly** runs on click.
- **LocalWrite** shows the dry-run plan inline, then runs on a second click.
- **RemoteWrite** and **DestructiveLocal** always go through a plan-then-apply screen: the full plan (files, remote steps, privacy, playlists), a button that names the exact action ("Upload to YouTube as unlisted", "Move 3 files to quarantine"), and for destructive actions what can be recovered. Applying sends the plan's digest; the server refuses if the plan it would now produce differs, so what runs is what was reviewed.
- Making a video public, moving an episode to another stage, and anything the engine marks remote stay separate, explicit actions. The GUI never chains them.

## Local Actions

Revealing a folder and opening a file in the desktop's default application are GUI-local actions that change no state. They are allowed only for paths inside the selected workspace, run only on an explicit click, and never run on page load or as a side effect of another action.

## Server

- Binds to `127.0.0.1` on a random free port by default; `--port` may fix it. It never binds to another interface.
- On start, prints the URL to stdout with a per-launch secret token in its fragment. Every API request must carry that token; the page keeps it in session storage.
- Rejects requests whose `Host` header is not the bound loopback address and port, and requests with a cross-origin `Origin`. No CORS headers are sent.
- Never sends credentials, tokens, or secret values to the page. Configuration already holds only credential references (a file path or an environment-variable name, `CredentialReference` in `src/channel.rs`); the GUI may show a reference but never reads the file or variable it points to.
- Watches the workspace's episode directories and pushes change notifications to the page, so the board and episode views update when files change outside the GUI. Notifications are debounced, because a filesystem synchroniser can produce bursts of changes.
- Serves the built frontend from the `eps` binary itself, so `cargo install` and release installers need no separate frontend step (G1).

## Frontend

React, TypeScript, Mantine and Vite in `gui/`, following the house-style frontend defaults and [ui.md](https://github.com/pacharanero/house-style/blob/main/ui.md), with one theme file and Lucide icons. Every view has loading, empty, error and disabled states. Keyboard operation covers the whole board and episode workflow, and the target is WCAG 2.2 AA. Before any milestone is complete, the rendered views are checked at desktop and narrow widths, with keyboard-only navigation and at 200% zoom.

## Milestones

| Item | Scope | Done when |
|---|---|---|
| R33 | Read and link: `eps gui` server, board, episode view, channel view, links panel, milestone derivation, model routing display, change notifications. No command execution except ReadOnly checks. | The board and episode views show real stages, milestones and links for a workspace, update when files change, and run read-only checks on request. |
| R34 | Local steps: jobs, queueing, dry-run then run for LocalWrite commands, per-run profile override (needs R36), previews, publicity drafts and post-URL recording. | A full post-production pass (process, transcribe, generate, check, compose thumbnail) can be run and reviewed from the GUI. |
| R35 | Remote steps: plan-then-apply screens with plan digests for upload, captions, thumbnail, playlists, description and publish; on-demand `youtube verify`. | An unlisted upload and its verification can be done from the GUI with the same guarantees as the CLI. |

## Open Questions

- **G1 - Shipping the frontend.** The built assets must reach crates.io and the release installers. Options: commit nothing built and have the release workflow build `gui/` and package `gui/dist` through Cargo's `include`; or build in `build.rs`, which would make `cargo install` depend on Node. The first is preferred.
- **G2 - Where job logs live.** In the workspace (for example a gitignored cache directory) so they sync with it, or in the user's state directory so they do not. Episode folders are not the place, because the artefact contract does not name them.
- **G3 - Staleness by time.** "Possibly stale" uses modification times, which a filesystem synchroniser can change. Recording input digests would be stronger but is canonical state the record does not yet carry (D1).
- **G4 - Channel-defined milestones.** How configuration describes policy milestones (calendar invitation, guest review, archive post) without channel material entering the engine (D2).
