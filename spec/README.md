# Episodic Specification

This directory defines the channel-neutral behaviour of `episodic`. Channel identity, editorial policy, branding, recording setup, destinations, credentials, and playlist choices do not belong here.

## Reading Order

1. [architecture.md](architecture.md) - product boundary, command surface, adapters, and safety model.
2. [filesystem-state.md](filesystem-state.md) - local persistence invariants and compatibility strategy.
3. [episode-lifecycle.md](episode-lifecycle.md) - episode stages and transition rules.
4. [artefact-contract.md](artefact-contract.md) - canonical episode artefacts and retention classes.
5. [decisions.md](decisions.md) - resolved design decisions and the reasoning behind them.
6. [documentation-map.md](documentation-map.md) - classification of the source workspace documentation.
7. [roadmap.md](roadmap.md) - implementation milestones and status.

Resolved decisions live in [decisions.md](decisions.md), whose codes match the stable query codes. Questions still open are raised in the parent workspace [`spec/queries.md`](../../spec/queries.md).

## Normative Language

`must`, `must not`, `should`, and `may` describe requirements in descending order of force. Current compatibility scripts are implementation evidence, not automatically normative behaviour. When a current script conflicts with this specification, the migration must record and test the deliberate choice.
