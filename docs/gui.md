# Browser workbench preview

The development preview shows every episode on a lifecycle board and lets you inspect each one. It cannot edit files, move episodes, generate metadata or publish anything yet.

- **Board**: one column per stage. Each card shows the thumbnail (once `<base>.thumbnail.png` exists), a strip of milestone marks, the next action, and YouTube and YouTube Studio buttons once a video ID is recorded. The header has the same two buttons for the channel when `[youtube] channel_id` is set.
- **Episode**: milestones with their detail, the thumbnail and frame candidates (select one to enlarge), artefacts with text previews, inspection findings, and a links panel with YouTube, recording and channel-destination links.
- **Publicity**: each platform section in the episode record, with its draft (or the title and watch link when there is no draft), a character count against `[metadata.social.<platform>-draft] max_characters`, a copy button, and a compose button for Bluesky, LinkedIn, X and Threads. Compose opens the platform's own composer; nothing is posted from the workbench.
- **Milestones** are derived from the files on every read. Filled squares are present, hollow are not started, a half-filled square is possibly stale (older than its input), a red circle is failing, and a dashed square is not applicable (for example media removed after publication). Hover a mark for its label, or open the episode for the full list. The same list appears in `eps episode inspect`.

The page checks every five seconds whether any episode file changed and reloads when one has, so work done in the terminal appears without pressing Refresh.

From the source checkout, install the frontend dependencies once:

```bash
npm --prefix gui ci --ignore-scripts
```

Then run against an explicitly selected channel:

```bash
s/gui-dev --config /path/to/channel/eps.toml
```

Open the private URL printed by the server in your browser. The fragment contains a per-launch access token; do not share that URL. It is removed from the address bar and held in the tab's session storage. The server listens only on `127.0.0.1`, rejects mismatched Host and Origin headers and has no mutation endpoint. Other same-user processes with access to your terminal or browser are outside this protection boundary.

`--port 8765` selects a fixed loopback port. Without it the OS selects a free port. Ctrl+C stops the server. No browser opens automatically, no remote services are contacted by the page (YouTube and compose links open only when you select them), and credentials are not read or returned. Local upload readiness is the existing CLI inspection result, not proof of remote publication or an independent media check.

Frontend assets are currently supplied through `--assets`, not embedded in release binaries. `s/gui-dev` builds them before running the source binary; a published `eps` version predating this work will not have the command. Packaging remains part of R33.

Run `s/lint`, `s/test`, `npm --prefix gui run build` and `npm --prefix gui audit` before committing. Browser smoke checks are documented in `gui/e2e/README.md`.
