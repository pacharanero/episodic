# Defects

Faults found in real use, with reproduction steps and the evidence that produced them. Item codes are stable and must not be renumbered or reused.

Legend: `[x]` fixed, `[~]` partially fixed, `[ ]` open

Roadmap milestones live in [roadmap.md](roadmap.md); resolved design decisions live in [decisions.md](decisions.md). This file records defects in implemented behaviour, which are neither.

## Source of these findings

All five were found in a single session on 2026-09-25, processing one real Everything Digital Health episode end-to-end with `eps 0.1.0`: a 21:45 1080p30 edited master through `audio analyse`, `audio process`, `transcript extract`, `transcript transcribe`, `metadata chapters|tags|quote|description`, `thumbnail frames|compose`, `episode new|promote|set|set-tags|mark-playlist|validate`, and `youtube plan-upload|upload|verify|playlist`, ending in a successful unlisted upload with captions, thumbnail, and two playlists.

This is the first end-to-end production run and therefore the beginning of R18 acceptance evidence. None of the defects below are architectural; each is a small, local fix.

What worked without incident, and is worth not regressing: `--dry-run` plans matched what applying did; the chapter task rejected its own first attempt for violating the contract and regenerated; `audio analyse` correctly attributed a post-processing true-peak overshoot to encoder behaviour and advised against reprocessing; `episode set` refused to overwrite non-placeholder values without `--force`; `doctor` was explicit about the limits of its own checks.

---

## F1 - Remote verification does not tolerate eventual consistency

- [ ] **Open.** Highest priority of these five.

`youtube upload` and `youtube playlist` write successfully, then immediately read back to confirm. The YouTube Data API is eventually consistent, so the read can return stale state and the command reports failure for work that in fact succeeded.

Observed twice in one run:

```
FAILED: captions uploaded but YouTube reports none
Error: some steps could not be confirmed; the video ID is recorded, so they can be retried
```

```
FAILED: playlist <ID> was added but YouTube does not report the video in it
Error: the repair could not be confirmed
```

Both had succeeded. `youtube verify` about 30 seconds later reported `Captions: 1 track(s)`; a further check about 45 seconds after the playlist call reported the video in both playlists. No retry was issued and nothing was re-uploaded.

**The verification itself is correct and must stay.** R10 deliberately records nothing the destination has not confirmed, and that invariant did its job: the video ID was written before anything else was attempted, so the 417 MB upload was never at risk and the remaining steps were retryable. The defect is only that confirmation is attempted once, immediately, with no allowance for propagation.

Consequences today:

- A fully successful upload exits non-zero, so any script or scheduled agent treating exit status as truth will conclude the upload failed.
- Confirmed steps are recorded and unconfirmed ones are not, so the episode record ends up incomplete after a run that actually completed. See F5 for how that compounds.
- Repeated false negatives train the operator to disbelieve real ones.

Suggested fix: bounded retry with backoff before declaring failure - poll confirmation a few times over roughly 30-60 seconds. Distinguish three outcomes rather than two: confirmed, not yet confirmed within the window (exit zero, record nothing, say plainly that it is unconfirmed and how to re-check), and positively contradicted by the destination (a real failure). Only the third should be a nonzero exit.

Reproduction: upload any episode with captions and a thumbnail using `youtube upload --apply`, then run `youtube verify` after 30 seconds.

---

## F2 - The transcript family is named so that the validator cannot find it

- [ ] **Open.**

The documented pipeline produces a captions file that `episode validate` reports as missing.

```
audio process <base>.mp4            -> <base>.processed.mp4
transcript extract <base>.processed.mp4 -> <base>.processed.wav
transcript transcribe <base>.processed.wav -> <base>.processed.srt (and .vtt/.txt/.json/.tsv)
```

`episode validate` then looks for `<base>.srt`:

```
- captions: missing (.../<base>.srt)
Findings:
- WARNING [missing-captions]: SRT captions are missing
```

The transcript basename is inherited from the input file, and the only input carrying processed audio is `<base>.processed.wav`. Transcribing the unprocessed master would give the right name but the wrong audio.

The existing channel archive uses `<base>.srt`, `<base>.wav` and so on with no `.processed.` infix, so the validator's expectation matches the established convention and the pipeline's output does not.

Worked around in the session by renaming the five transcript files plus the wav; validation then reported `Findings: none`.

Suggested fix: strip a `.processed` infix when deriving the transcript basename, or give `transcript transcribe` an explicit basename option. Either way `episode validate` and the emitted filenames must be decided in one place, since today they disagree.

Reproduction: run the three commands above on any episode, then `episode validate`.

---

## F3 - Transcripts are not clamped to media duration, so trailing hallucinations survive

- [ ] **Open.**

Whisper emitted ten trailing cues over the silent tail of the episode, the last of them ending 29 seconds after the end of the media file.

Media duration was 1305.60 s (21:45.6). The transcript ran to 22:14.88, and `metadata chapters` duly reported `Transcript: 39 blocks, ending at 22:14.` Cues 224-233 were `Bye.` repeated eight times and `Thank you.` twice, two of them spanning 30 seconds each.

Measured independently: audio from 1255 s onward was digital silence at -91 dB mean and peak. Real speech ended at 1244.74 s. Every one of the ten cues sat over silence, and the final cue lay entirely beyond the end of the file.

This is a well-known Whisper failure mode on trailing silence, not a fault in the transport. But the artefact is published state, so the engine should not pass it through unexamined: the cues are wrong, they would ship as captions, and a downstream chapter generator reads them as content.

Cleaned by hand in the session across all five formats (233 -> 223 cues), after which chapters were regenerated from the corrected transcript.

Suggested fix: after transcription, drop cues starting at or after media duration, and trim a trailing run of short repeated identical cues whose span is silent. Report what was removed on stderr rather than doing it silently. Media duration is already available via ffprobe, which `doctor` confirms is present.

Reproduction: transcribe any episode with more than about 30 seconds of silence or an end card after the last speech.

---

## F4 - `thumbnail compose` has no `--force` and silently no-ops

- [x] **Fixed.** `--force` recomposes over a finished pair, matching `TaskArgs`.

Recomposing a thumbnail with a different frame or caption does nothing if the output already exists:

```
$ eps thumbnail compose <frames>/frame-02.jpg "inexcusable" --output <base>.thumbnail.png
Already done: <base>.thumbnail.png, <base>.thumbnail.svg
```

The message is accurate but the command is one an operator will repeatedly want to re-run, since choosing a frame and a caption is iterative by nature. The `metadata` tasks already carry `--force` for exactly this; `thumbnail compose` does not, so the only route is deleting the PNG and the SVG by hand.

Suggested fix: add `--force` to `thumbnail compose`, matching `TaskArgs`. Note that two files must be removed, so operators working around it will miss the SVG.

Fixed as suggested. Both members are now staged before either is published, because recomposing over a finished pair could otherwise replace the SVG and then fail to replace the PNG, leaving a pair that looks complete but whose caption and image disagree. That window did not matter while the only reachable case was composing from nothing.

---

## F5 - `episode mark-playlist` depends on state that a failed step would have written

- [ ] **Open.**

`mark-playlist` requires the playlist to be present in the record already:

```
Error: no playlist entry mentioning '<ID>' in .../<base>.episode-data.md
```

This is correct on its own terms - R6 deliberately errors on an unknown playlist rather than inventing a line. The problem is the interaction with F1. The skeleton from `episode new` lists the configured standard playlists and a `- [ ] [TBD]` placeholder for topic playlists. The topic entry is written by a successful `youtube playlist` run. When that run reports a false failure, nothing is written, and the manual repair path is then refused because the entry it needs is missing.

The operator is left editing the Markdown by hand, which is precisely what these commands exist to avoid.

Fixing F1 removes the common trigger. Worth considering independently: when `youtube playlist --apply` targets a playlist named in channel configuration, it could add the entry to the record unticked at the point of the API call, leaving `mark-playlist` free to tick it once confirmed. That keeps the invariant that only confirmed facts are ticked, while ensuring the row the repair path needs exists.

Reproduction: add a video to a topic playlist that is not yet listed in the episode record, then attempt `episode mark-playlist`.

---

## Not defects

Recorded so they are not re-reported.

**Diagnostic lines appearing in redirected artefacts.** `metadata` tasks print the model, attempt number, and any rejection reason to stdout alongside the generated text, so `eps metadata tags ... > file` captures them. This was initially mistaken for a stdout/stderr violation. It is not: `--output` writes the artefact alone, cleanly, and is the intended route. Shell redirection is simply the wrong way to call these commands. Worth a line in the command help, since the mistake is easy to make.

**True peak above ceiling after `audio process`.** Reported as `-1.2 dBTP (ceiling -1.5) over` on a correctly normalised file. The tool already explains this is encoder overshoot after measurement and advises against reprocessing. Correct behaviour.

**`transcription.device` defaulting to CPU.** Channel configuration, not an engine fault. Worth noting only that the difference on the test machine was about 1 minute 41 seconds on CUDA against an estimated 15-20 minutes on CPU for a 21-minute episode, so the default is expensive where a GPU is present.

**`metadata description` requiring `--transcript`.** Behaved as documented; the flag exists on `TaskArgs`. Initially misread from truncated help output.
