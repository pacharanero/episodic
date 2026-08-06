# Artefact Contract

## Naming

Every durable episode artefact shares the episode-folder basename `<base>` so a file remains attributable when viewed in isolation.

| Artefact | Canonical name | Class |
|---|---|---|
| Episode state | `<base>.episode-data.md` during compatibility phase | Durable state |
| Show notes | `<base>.md` | Durable human content |
| Edited master | `<base>.mp4` | Review-retained media |
| Processed/upload master | `<base>.processed.mp4` | Durable publication media when uploaded |
| Extracted audio | `<base>.wav` | Reproducible intermediate |
| SubRip captions | `<base>.srt` | Durable transcript/captions |
| WebVTT captions | `<base>.vtt` | Durable transcript/captions |
| Plain transcript | `<base>.txt` | Durable transcript |
| Segment data | `<base>.json`, `<base>.tsv` | Durable or reproducible transcript data |
| Chapters | `<base>.chapters.txt` | Durable publication metadata |
| Tags | `<base>.tags.txt` | Durable publication metadata |
| Description | `<base>.youtube-description.txt` | Durable publication metadata |
| Thumbnail candidates | `<base>.thumbnail-quotes.txt`, `<base>.frames/` | Review/transient |
| Final thumbnail | `<base>.thumbnail.png`, `<base>.thumbnail.svg` | Durable publication asset |

Channel adapters may add named destination artefacts, but they must not silently replace these core meanings.

## Validation

- Required artefacts must be non-empty and parseable in their declared format.
- Video and audio validation uses FFprobe rather than filename extension alone.
- Caption timestamps must be ordered and end near the media duration.
- Chapter markers must be ordered, begin at zero, and cover the episode rather than only a truncated prompt.
- Tags and descriptions obey destination length and structure limits supplied by the destination adapter or channel policy.
- Thumbnail dimensions and encoding follow channel/destination policy.

## Atomicity And Idempotency

Generators stage into a same-directory temporary path, validate the result, and then rename it into place. Existing valid outputs are skipped unless `--force` is supplied. Empty, partial, or invalid outputs are replaceable failures, never idempotency markers.

## Retention

Keep human content, canonical state, transcripts, captions, descriptions, thumbnails, publication records, and the exact uploaded video. Raw recordings, alternate renders, extracted audio, frame candidates, editor caches, and proxies are review candidates after the channel retention period. Plans distinguish reproducible clutter from potentially useful source material and require explicit approval before moving either.
