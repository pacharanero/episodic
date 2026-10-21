# Episodic Guides

These guides describe the installed `eps` workflow. They complement the normative specifications in [`spec/`](../spec/README.md) and the channel configuration reference in [`channels/`](../channels/README.md).

- [Getting started](getting-started.md) installs `eps`, creates a local channel workspace, and creates an episode.
- [First episode](first-episode.md) follows an episode through the local production workflow and a guarded YouTube upload.
- [YouTube setup](youtube-setup.md) describes the existing authorised-user credential model and the explicit remote-write safeguards.
- [Troubleshooting](troubleshooting.md) starts with `eps doctor` and explains the limits of its local checks.
- [Beyond `eps`](beyond-eps.md) covers production work `eps` deliberately leaves to other tools - video overlays, extra thumbnail compositing, and what to do when an uploaded video's content itself needs to change.

For the complete configuration shape, copy only the sections needed from [`channels/example.toml`](../channels/example.toml). Production configuration, credentials, media, and episode records belong in a separate channel workspace and must not be committed to this repository.
