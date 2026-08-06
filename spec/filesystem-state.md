# Filesystem State

## Invariant

All durable state remains in ordinary local files. A channel workspace can be synchronised by an external filesystem synchroniser, backed up, inspected, searched, and edited without an `episodic` server. No database is introduced.

## State Classes

- **Channel configuration**: TOML containing identity, paths, policy, branding, destinations, adapter profiles, and non-secret credential references.
- **Episode state**: one canonical record inside each episode folder, plus human-authored show notes and generated artefacts.
- **Plans**: explicit serialisable descriptions of proposed local or remote mutations.
- **Caches**: reproducible remote snapshots or generated indexes kept outside canonical state and safe to rebuild.
- **Secrets**: environment files, operating-system key stores, or existing OAuth stores; never canonical state.

## Canonical Episode Format

Legacy `<base>.episode-data.md` remains the canonical episode record for now, and the choice of a new serialization is deliberately deferred until real usage shows what the format has to carry (D1). The Rust domain model must not depend on JSON, YAML, TOML, or Markdown details, so that the decision stays cheap to make later.

The compatibility adapter currently reads labelled fields without writing them. Read-only support came first so inspection and conformance fixtures could exist before any migration. The next step is lossless writes back to the same Markdown: `eps` updates only the fields it owns and preserves all unknown human-authored prose, exactly as the current `edh-*` scripts must. Markdown therefore stays authoritative for the EDH workspace, and no migration is planned until a format decision is actually taken.

### Episode Schema Shape

The canonical record is a small generic core plus typed integration records (D4), matching the channel-configuration split:

- **Core**: identity, dates, participants, lifecycle stage, durations, artefact references, and generated metadata that every channel has.
- **Integrations**: destination-specific records such as YouTube publication facts, playlist selections, social posts, and archive entries.

A channel that does not use a destination carries no record for it. Everything Discourse must not inherit EDH-only fields, and EDH's current legacy fields survive as compatibility fixtures rather than as universal schema.

## Preservation

- Readers tolerate missing optional sections and known legacy shapes.
- Writers update only owned fields and preserve unknown human-authored content.
- Generated files are written to a same-directory temporary file, validated, then atomically renamed.
- Failed external actions do not change canonical state.
- Remote state is recorded only after verification, not from optimistic command intent.
- The exact video supplied to the successful upload is recorded as an immutable publication fact.

## Workspace Discovery

Explicit paths take precedence. When no explicit path is supplied, discovery may walk through documented channel configuration locations but must not silently cross an enclosing workspace boundary. Every resolved path must be available in structured output.

Path arguments expand a leading `~` inside `eps`, including quoted and `--flag=~/...` forms that the shell does not expand.
