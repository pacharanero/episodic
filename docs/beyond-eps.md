# Beyond `eps`

`eps` orchestrates local tools; it does not reinvent them. A production session commonly needs work `eps` deliberately leaves to other tools. This guide is for an agent (or a human) doing that work, so it reaches for the right tool instead of fighting `eps` or hand-rolling something `eps` already does better.

## What `eps` does not do

- Record or edit video. There is no cutting, trimming, re-ordering, or mid-timeline text overlay. The `edited_video` artefact (`<base>.mp4`) is expected to already exist, produced by whatever editor the channel uses (Blender, Kdenlive, anything else).
- Composite images beyond one thing: a single caption overlaid on one chosen frame (`eps thumbnail compose`). It does not place a second image (a logo, a watermark) onto that frame for you.
- Replace the file content of an already-uploaded YouTube video. The YouTube Data API has no such operation; see [Replacing an uploaded video](#replacing-an-uploaded-video) below.
- Post to social platforms, manage calendars, or do anything outside local production and the YouTube Data API.

None of this is a gap to work around by scripting `eps` into doing it anyway. Use the external tool directly, then hand the result back to `eps` at the next artefact boundary it does understand.

## Recipe: burning a corrective text overlay into processed video

A guest asks for a short on-screen correction or clarification after reviewing an unlisted upload. `eps` has no overlay command; `ffmpeg` does.

1. Find the exact wording and timing from the episode's own transcript (`<base>.srt`), not from a rough guess. Grep the phrase, read the surrounding cues, and use the SRT timestamps directly.
2. Burn the text into `<base>.processed.mp4` (the artefact `eps youtube upload` actually uploads), not the pre-audio `edited_video` master. Re-editing in the original video editor and re-rendering would also discard the already-optimised audio, forcing a second pass through `eps audio process` for no reason.
3. Re-encode video only; copy the audio stream untouched so the existing optimisation and caption timing both survive:

   ```bash
   ffmpeg -i <base>.processed.mp4 \
     -vf "drawtext=fontfile=/path/to/font.ttf:textfile=overlay.txt:fontcolor=white:fontsize=46:box=1:boxcolor=black@0.65:boxborderw=24:x=(w-text_w)/2:y=h-230:enable='between(t,T1,T2)'" \
     -c:v libx264 -profile:v high -level 4.0 -pix_fmt yuv420p -crf 18 -preset medium \
     -c:a copy \
     <base>.processed.overlay.mp4
   ```

   Match `-profile`, `-level`, and `-pix_fmt` to the source (`ffprobe -show_entries stream=profile,level,pix_fmt`) so the replacement is a drop-in. Use `textfile=` rather than an inline `text=` argument; it avoids escaping commas and colons in filter-graph syntax entirely.
4. Before trusting it, extract a frame from the new file at the overlay's timestamp and actually look at it. Check it doesn't sit on top of something the overlay would obscure (a face, on-screen content being discussed).
5. Replace `<base>.processed.mp4` with the new file. Keep the original alongside (for example `<base>.processed.pre-overlay.mp4`) rather than discarding it.
6. See [Replacing an uploaded video](#replacing-an-uploaded-video) for what happens next if this episode was already uploaded.

## Recipe: compositing a second image onto a thumbnail frame

A guest supplies a logo to use instead of their face, or any other image that needs to sit on a candidate frame before `eps thumbnail compose` adds the caption.

1. Extract candidate frames as usual: `eps thumbnail frames <base>.processed.mp4`.
2. Rasterise a vector asset at a size larger than its final placement (`rsvg-convert -w 900 -h 900 logo.svg -o logo.png`); composite from a bitmap, not by guessing ImageMagick's own SVG rendering.
3. Composite onto the chosen frame with ImageMagick, then look at the result before guessing further:

   ```bash
   magick frame-06.jpg logo.png -geometry +X+Y -composite frame-06-logo.jpg
   ```

   Getting the placement right is iterative: composite, read the image back, adjust the offset, repeat. Don't try to compute exact coordinates from the source image alone; crop a region around the target area first to measure it.
4. Feed the composited frame into `eps thumbnail compose` as normal, so the channel's own caption styling (font, stroke, uppercase policy, quoting) still applies consistently with every other thumbnail:

   ```bash
   eps thumbnail compose frame-06-logo.jpg 'Caption text' --output <base>.thumbnail.png --force
   ```

## Overriding one channel-policy field for a single command

A one-off needs a different setting than the channel's configured default (a guest's brand name wants to deviate from the uppercase-caption policy; a GPU is temporarily unavailable and this one transcription needs `device = "cpu"`). Do not edit `eps.toml` for this. Copy it, change the one field, pass `--config` to the single command that needs it, then delete the copy:

```bash
sed 's/uppercase = true/uppercase = false/' eps.toml > .eps-override.toml
eps --config .eps-override.toml thumbnail compose frame.jpg 'Caption text' --output <base>.thumbnail.png --force
rm .eps-override.toml
```

Keep the override file inside the channel workspace (so relative asset paths such as `font_path` still resolve) and remove it immediately afterward; never leave a second configuration file lying around for a later session to find and trust by accident.

A display font may not visually distinguish case at all (several bold condensed/display faces, Bungee among them, are effectively unicase). Check the rendered output before concluding an override worked.

## Replacing an uploaded video

The YouTube Data API has no operation to replace a video's file content in place. `videos.update` only changes metadata: title, description, tags, thumbnail, captions, privacy. If the actual video (or audio) content needs to change after upload, the only path is to delete the existing video and upload the new file fresh, which creates a new video ID and a new URL.

Before deleting anything:

- Confirm with whoever shared the old link that a broken link is acceptable. An unlisted review link already sent to a guest is usually fine to replace; a link already circulated more widely usually is not.
- `eps` refuses to upload an episode whose `<base>.episode-data.md` already records a video ID, to prevent an accidental duplicate upload:

  ```
  Blocked:
    already recorded as uploaded (VIDEO_ID); uploading again would create a duplicate
  ```

  If the old video is genuinely gone, reset the recorded YouTube fields (`Video ID`, `YouTube URL`, `YouTube Short URL`, `Privacy Status`, `Upload Date`, `Captions`) back to `[To be added]` in `episode-data.md` before `eps youtube upload` will proceed again.
- Deleting the video itself is outside `eps`'s current command surface (see F8-adjacent gap in `spec/roadmap.md`); use the YouTube Data API directly or a tool that wraps it.

## Local tool availability can fail independently of `eps`

`eps doctor` checks that a configured tool is present on `PATH`; it does not prove the tool can actually run (a GPU-backed transcription device, for example, can be configured and present on `PATH` while the GPU driver itself is in a broken state — `nvidia-smi`/NVML failing is a system-level fault, not an `eps` or Whisper bug). Don't attempt to fix host drivers or reboot a shared machine without asking first; fall back to a software path (`--device cpu`, via the override pattern above) for the one run that needs to proceed now, and leave the underlying fix to whoever owns the machine.
