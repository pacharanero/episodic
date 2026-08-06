# Episode Lifecycle

## Stages

```text
in-planning -> in-progress -> edited -> published
                                  \-> not-for-publishing
```

An episode folder belongs to exactly one lifecycle directory. Its basename is `YYYY.MM.DD-slug`, and its canonical state file uses the same base. Folder location expresses lifecycle stage; duplicating one episode across stage directories is invalid state.

## Pre-Production

Planning captures title, intended date, presenters and guests, recording arrangements, research, and show notes. Channel policy determines calendar integration, guest joining details, and approval requirements.

## Intra-Production

Recording produces one or more source media files. The episode enters `in-progress` when recording or active production begins. Recording software and hardware are channel/operator concerns, while `eps` may provide read-only preflight and artefact inspection.

## Post-Production

Editing produces a canonical edited master. Deterministic processing may then produce an upload master, transcript family, chapter markers, tags, description, frame candidates, and thumbnail. An episode is upload-ready only after required artefacts pass their contracts; file presence alone is insufficient.

## Upload And Review

Upload is a remote write that defaults to unlisted and requires `--apply`. Captions, thumbnail, and each selected playlist are separate verifiable outcomes. Local state records confirmed results independently so a partial failure is representable and retryable.

Guest review policy is channel-specific and is not machine-enforced (D5). `eps` must not require a recorded guest-approval field before a public publish. Making a video public is already an explicit human action requiring `--apply`, and that is where the policy is honoured. A channel may still record review state as episode data; `eps` reports it without gating on it.

## Publication

Making a remote item public is separate from upload. Promotion to `published` follows remote verification and records publication date, privacy, destinations, and the exact uploaded source. A public remote item in another local stage is a reconciliation warning, not an assumption that a move succeeded.

## Post-Publication

Publicity, archive posting, index regeneration, retention review, and cleanup are independent operations. Each reports its own verified result. Cleanup is never an implicit side effect of publication.
