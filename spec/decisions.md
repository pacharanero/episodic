# Decisions

Resolved design decisions that are not obvious from the resulting code, with the reasoning that produced them. Decision codes match the query codes used in the parent EDH workspace `queries.md` and are stable. A decision is revisited by adding a new entry, not by silently editing an old one.

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
