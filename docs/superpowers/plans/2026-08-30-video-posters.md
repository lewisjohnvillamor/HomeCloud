# Video Poster Frames Implementation Plan

> The last gap in Phase 4. Videos currently show a generic file icon in Files
> and are absent from Photos, so a library of holiday clips is a wall of
> nothing.

**Goal:** A video shows a still frame from itself wherever a photo would show a
thumbnail, without the server ever handing an untrusted file to a decoder that
can run unbounded.

**Architecture:** FFmpeg as an external process — not a linked library — so a
crash or a hang is contained in a child the server can kill. Frames go through
the same derivative cache and the same thumbnail endpoint as images.

## Tasks

1. **`crates/media/video`** — probe duration, extract one frame, encode JPEG,
   all via an FFmpeg child process with a wall-clock timeout, bounded output,
   no shell, and no network.
2. **Availability** — FFmpeg is optional. Where it is missing, videos report no
   preview and everything else works, the same way passkeys behave without a
   public origin.
3. **Thumbnails** — the existing endpoint serves video posters from the same
   cache, keyed the same way.
4. **UI** — videos get a poster in the file list and appear in Photos.
5. **Adversarial tests** — a file that is not a video, a truncated video, a
   file that claims to be a video, and a video that would take too long.

## Self-Review

- Transcoding for playback is a different feature with a different resource
  model (long-running, per-viewer) and is not in this plan.
- One frame at a fixed offset, not a chosen "best" frame: choosing needs
  scene detection, which is a model, and the point of this plan is that it
  works with AI disabled.
