# Browser workbench preview

The development preview shows episode stages, local inspection findings, artefact presence, recorded YouTube links and configured model routing. It cannot edit files, move episodes, generate metadata or publish anything yet. Refresh reads the files again; automatic change notifications are not implemented.

From the source checkout, install the frontend dependencies once:

```bash
npm --prefix gui ci --ignore-scripts
```

Then run against an explicitly selected channel:

```bash
s/gui-dev --config /path/to/channel/eps.toml
```

Open the private URL printed by the server in your browser. The fragment contains a per-launch access token; do not share that URL. It is removed from the address bar and held in the tab's session storage. The server listens only on `127.0.0.1`, rejects mismatched Host and Origin headers and has no mutation endpoint. Other same-user processes with access to your terminal or browser are outside this protection boundary.

`--port 8765` selects a fixed loopback port. Without it the OS selects a free port. Ctrl+C stops the server. No browser opens automatically, no remote services are contacted by the page, and credentials are not read or returned. Local upload readiness is the existing CLI inspection result, not proof of remote publication or an independent media check.

Frontend assets are currently supplied through `--assets`, not embedded in release binaries. `s/gui-dev` builds them before running the source binary; a published `eps` version predating this work will not have the command. Packaging remains part of R33.

Run `s/lint`, `s/test`, `npm --prefix gui run build` and `npm --prefix gui audit` before committing. Browser smoke checks are documented in `gui/e2e/README.md`.
