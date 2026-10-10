# Decisions

Resolved design decisions that are not obvious from the resulting code, with the reasoning that produced them. Decision codes match the query codes used in the parent EDH workspace `spec/queries.md` and are stable. A decision is revisited by adding a new entry, not by silently editing an old one.

## D1 - Canonical episode format is deferred

Decided 2026-08-06. Legacy `<base>.episode-data.md` remains the canonical episode record. A new serialization (JSON, YAML, TOML) is not chosen now, because the right format only becomes clear once `eps` is doing real work and we can see what the record actually has to carry.

Consequences: the Rust domain model must stay independent of any serialization. The Markdown adapter graduates from read-only to lossless read/write rather than being replaced. No migration is planned.

See [filesystem-state.md](filesystem-state.md).

## D2 - No channel-specific material in this repository

Decided 2026-08-06. `episodic` is intended for publication as a general-purpose tool that others can use, so it carries nothing specific to any of Marcus's channels. It holds the schema, `channels/example.toml`, and redacted conformance fixtures. Production channel configuration, editorial policy, credentials, and episode state live in each channel's own workspace.

Consequences: EDH configuration and channel documents moved to the EDH workspace under `channels/everything-digital-health/`. Extraction to a standalone repository is a move, not a cleanup.

See [architecture.md](architecture.md).

## D3 - Copyright is Dr Marcus Baw and Koloki Ltd

Decided 2026-08-06. `episodic` is not healthcare-specific, so it is not attributed to the healthcare company. Copyright is `Dr Marcus Baw and Koloki Ltd`; licensing remains AGPL-3.0-or-later for code and CC-BY-SA-4.0 for documentation.

## D4 - Generic core plus typed integration records

Decided 2026-08-06. Both channel configuration and the episode schema are a small generic core plus typed records for the integrations a channel happens to use, rather than promoting the current EDH-shaped record to the universal schema.

Consequences: a future channel inherits no EDH-only fields. EDH's existing fields survive as compatibility fixtures. Unknown top-level configuration sections are rejected rather than ignored, so a typo fails validation instead of silently disabling policy.

See [filesystem-state.md](filesystem-state.md) and [../channels/README.md](../channels/README.md).

## D5 - Guest approval is not a machine gate

Decided 2026-08-06. Participant veto remains real editorial policy, but `eps` must not require a recorded approval field before a public publish. Making a video public is already an explicit human action requiring `--apply`, and that is where the policy is honoured. A channel may record review state as episode data; `eps` reports it without gating on it.

See [episode-lifecycle.md](episode-lifecycle.md).

## D6 - Second channel deferred

Decided 2026-08-06. Work on Everything Discourse has not started, and its configuration, editorial policy, and acceptance episode must not be invented ahead of a real one.

Consequences: channel neutrality is defended in the meantime by D2 and D4, not by a second live channel. Release readiness still requires a real second channel proved through configuration alone.

## D7 - Audio port reproduces the current single-mix chain

Decided 2026-08-06. When audio processing moves into `eps`, it reproduces the working single-mix chain exactly: `highpass -> afftdn -> dynaudnorm -> two-pass loudnorm`, as implemented in the EDH workspace `tools/production/edh-process-audio`. The older unshipped multi-track/profile design is not being built: it never shipped, its comparison evidence is missing, and the local chain already does substantially what the external service did. That service was only ever used to rescue two episodes. Its specification has been deleted so it cannot be mistaken for a plan.

See [architecture.md](architecture.md).

## D8 - Talk to YouTube directly through google-youtube3

Decided 2026-08-07. `eps` will use the `google-youtube3` crate rather than orchestrating the `yutu` CLI.

Reasoning. Only seven operations are actually used: `video insert`, `video update`, `video list`, `playlistItem insert`, `playlistItem list`, `caption list`, and `thumbnail set`. Against that small surface, the CLI was costing more than it returned:

- Caption upload already bypasses `yutu` entirely and posts to the API with `curl`, because `caption insert` returns persistent `invalidMetadata` errors. The hardest operation was already hand-rolled.
- The workspace runs a `yutu` fork carrying two unupstreamed patches, one of which removes a tag `yutu` injects into published videos.
- `yutu` does not reliably emit clean JSON: publication code greps its output with a regular expression because authentication messages are sometimes printed before the JSON.
- `yutu` is growing into an AI toolkit, MCP server, and agent, so its surface changes for reasons unrelated to these seven calls.

Authentication is not a reason to keep it. The stored token is a plain OAuth2 token with a refresh token, and the client secret carries the token endpoint, so refresh is an ordinary form POST. The interactive consent flow has already been completed and does not need repeating.

`google-youtube3` was chosen over a hand-written adapter to get schema regeneration and the wider API surface without further work, accepting that it brings tokio and hyper and therefore makes `eps` async. Binary size and the async runtime are acceptable costs.

Consequences: R22 targets a native adapter rather than `yutu` orchestration, and its resumable upload path replaces the current all-or-nothing upload. Upstreaming the `yutu` patches leaves the critical path but stays worth doing for other users. The existing refresh token is reused; if the Google Cloud project is ever returned to Testing status, refresh tokens expire after seven days.

## D9 - Standalone source and explicit workspace selection

Decided 2026-09-07. Extraction to a standalone repository is approved. The normal workflow is an installed `eps` invoked in a channel workspace containing `eps.toml`; elsewhere the caller supplies `--config`. Selection is explicit CLI path first, then PWD `eps.toml` only. Omit the unresolved `EPS_CONFIG` selector. No ancestor or global fallback, `--channel` selection, or nested-layout inference remains. This deliberately narrows the house-style discovery hierarchy to prevent accidentally selecting a different channel.

Relative assets use the normalized absolute config parent, and every normal load runs semantic validation as well as strict TOML parsing. `channel list` represents the single selected workspace, not a multi-channel registry. Config-independent commands such as help, version, and explicit episode inspection need no workspace config.

Migration is tolerant and iterative at the workflow and legacy-document boundary, not at the config validation boundary. Existing configs can be explicitly selected after their relative asset paths are adjusted; no automatic rewrite is added. External channel scripts and installation are separate work. Doctor, release automation, and clean-machine distribution remain future tasks. D8's native YouTube adapter is implemented as R10; its old R22 reference is historical, and `yutu` is not an engine dependency.

## D10 - Distribution home, registry, and channels

Decided 2026-10-10.

**The repository is `pacharanero/episodic`.** The copyright holders are Dr Marcus Baw and Koloki Ltd (D3), so the owning account was genuinely ambiguous between the personal and company namespaces. It is published under the personal account because `episodic` is a general-purpose tool rather than a company product, and D2 already separates it from any channel or company it happens to serve.

**It publishes to crates.io as `episodic-cli`.** `eps` is a CLI people may reasonably `cargo install`, and the library is the engine a future GUI and MCP server will share, so the registry is the natural distribution point as well as a name claim worth making early. The shorter `episodic` name is already owned by an unrelated project, so the published package name distinguishes this CLI while the project, library, and installed binary keep their existing names. Publication is by OIDC trusted publishing rather than a stored registry token, so no long-lived credential sits on the repository. Consequence: the `episodic-cli` name has to be owned on crates.io with a trusted publisher configured for `publish-crates.yml` before the first release can complete, and a published version can only ever be yanked, never withdrawn, which is why the publish workflow runs the tests even though the release build does not.

**Homebrew is deferred.** The house standard says to set `tap` and `publish-jobs` only once a maintained Homebrew channel exists, and there is no tap for this project yet. The shell, PowerShell, and MSI installers plus `cargo binstall` cover the platforms in the meantime. Revisit at the next distribution review; adding it later is two config lines and a tap token, and needs no change to the release model.

See [roadmap.md](roadmap.md) R17 and `~/code/house-style/distribution.md`.
