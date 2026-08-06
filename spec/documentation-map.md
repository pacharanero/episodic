# Source Documentation Map

This map records the review of project-relevant Markdown in the containing Everything Digital Health workspace. It separates durable channel-neutral decisions from channel policy and live state without copying obsolete runbooks into the new project. Dependency documentation, generated transcripts, routine episode show notes, and the embedded episode demo project are intentionally excluded.

The original documents remain in place while current `edh-*` scripts operate. A source document is removed or reduced only after its replacement is proven and links are updated.

## Generic Tooling Sources

| Source in parent workspace | Disposition |
|---|---|
| `spec/episodic-cli.md` | Distilled into `architecture.md`, `filesystem-state.md`, `episode-lifecycle.md`, and `roadmap.md`. |
| `channels/README.md` | Generic configuration concepts belong in `../channels/README.md`; concrete EDH values remain channel-specific. |
| `prompts/README.md` | Future contributor documentation for prompt contracts and overrides; not yet migrated because prompt execution has not moved. |
| `prompts/chapters.md`, `bluesky-draft.md`, `mastodon-draft.md`, `linkedin-draft.md`, `instagram-caption.md` | Candidate default prompt contracts; migrate with the metadata command family. |
| `spec/api-docs-youtube.md` | Future integration reference; `eps` will link to official YouTube and `yutu` docs rather than own a copied API guide. |
| `CLAUDE.md` | Replaced by the thin pointer in this project. |

## Mixed Sources Split By Boundary

| Source in parent workspace | Generic destination | Channel destination |
|---|---|---|
| `README.md` | Project purpose, lifecycle concepts, local-state invariant | EDH commands, paths, credentials, social destinations, retention values |
| `AGENTS.md` | Core invariants and validation workflow | EDH workspace instructions and current compatibility commands |
| `roadmap.md` | Product milestones in `roadmap.md` | EDH compatibility and cutover tracking remains in the parent workspace |
| `spec/overview.md` | Lifecycle in `episode-lifecycle.md` | EDH's actual OBS, calendar, YouTube, publicity, and archive steps |
| `spec/tooling.md` | Composition, atomicity, naming, artefacts, adapters | Current `edh-*` inventory, credentials, model choice, audio settings, and known upload gaps |
| `spec/episode-metadata-contract.md` | Preservation and managed-state principles in `filesystem-state.md` | EDH-specific legacy fields remain compatibility fixtures |
| `spec/toolchain-requirements.md` | Future prerequisite/doctor contract | Current machine paths and EDH setup remain external |
| `youtube-upload.md` | Generic plan/apply/verify sequence for a future YouTube adapter | EDH IDs, boilerplate, tags, manual Studio policy, and break-glass commands |
| `introduction.md` | None | Editorial policy in the EDH workspace `channels/everything-digital-health/docs/editorial-policy.md`; stale service details and episode links remain external state |
| `copypasta.md` | None | Any approved disclaimer belongs in EDH editorial policy; scratch tag lists are obsolete |
| `prompts/tags.md`, `youtube-description.md`, `thumbnail-quote.md`, `guest-research.md` | Output contracts | EDH/NHS/UK voice, hashtags, and host choices become configuration or overrides |

## Channel-Specific Sources

| Source in parent workspace | Channel destination |
|---|---|
| `design.md` | the EDH workspace `channels/everything-digital-health/docs/brand.md` |
| `spec/recording-preflight.md` | the EDH workspace `channels/everything-digital-health/docs/recording-preflight.md` |
| `vdo-ninja/vdo-ninja-setup.md` | EDH recording setup and channel configuration |
| `youtube-automation-setup.md` | the EDH workspace `channels/everything-digital-health.toml` and publication policy |

## Live State, Not Product Documentation

| Source group in parent workspace | Treatment |
|---|---|
| `queries.md` | Active decision register while this project remains nested. |
| `planned-episodes.md` | EDH editorial backlog; remains in the channel workspace. |
| `episodes/index.md` | Generated EDH lifecycle index; remains in the channel workspace. |
| `episodes/**/<base>.episode-data.md` | Live episode records; retain externally and copy only curated, redacted compatibility fixtures. |
| `episodes/in-planning/cwc-quick-start-video-plan.md`, `map_of_medicine_intellectual_property_controversy.md`, `terminologies-season.md` | EDH content planning; remains external. |
| `playlists/*.md` | Generated YouTube cache; future location is a rebuildable workspace cache, not source documentation. |
| `tools/tests/fixtures/episode-data/canonical.episode-data.md` | Conformance input; a representative fixture is migrated under `tests/fixtures/`. |

## Legacy Or Superseded Sources

| Source in parent workspace | Treatment |
|---|---|
| `spec/portable-toolset.md` | Historical shell-tool architecture superseded by `episodic`; do not migrate. |
| `spec/audio-optimisation.md` | Superseded by the implemented single-mix chain (D8). Its unshipped multi-track/profile design is not being built; retain the document only as background. |
| `tags/tag-generation-guide.md`, `tags/edh-yt-tag-generation-from-description.md` | Superseded by prompt contracts and validators; retain only useful examples as future fixtures. |
| `transcription/README.md` | Legacy standalone environment; salvage backend requirements during transcription adapter work. |
| `edh-gui/spec.md` | Prototype backend is superseded; retain only the filesystem-watch/SSE and workbench UX lessons for the future GUI specification. |
| Episode-local `POST-UPLOAD-TASKS.md` and `UPLOAD-SUCCESS.md` files | Historical episode evidence; no project migration. |

## Nested `yutu` Documentation

`yutu/README.md`, `yutu/docs/FEATURES.md`, and `yutu/skills/youtube/references/{setup,workflows}.md` define the external adapter's supported surface and remain owned by `yutu`. `episodic` should link to them and test required capabilities rather than copy them. `yutu`'s autonomous agent instruction prompts and SEO guide do not define `eps` behaviour and are not migrated.

## Missing Or Stale References Found During Review

- `spec/auphonic-analysis-2026-03-31.json` is referenced but absent.
- `edh-gui/spec.md` references absent `get_playlists.py` and `yutu-quick-reference.md`.
- During review, the parent `AGENTS.md` said deleted `edh-cli/` was retained; it now points to `episodic/` and explicitly forbids reviving the old project.
- `episodes/index.md` attributes generation to stale path `tools/edh-list-episodes`; the implementation is under `tools/planning/`.
- `youtube-upload.md` calls itself the only instruction file while `edh-finalise-episode` says it replaces its first three steps.
- No real Everything Discourse configuration or policy documentation exists yet. Work on that channel has not started (D6), so nothing may be invented for it.
