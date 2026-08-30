# Image Derivatives Implementation Plan

> Follows `2026-08-29-catalog-transfers-and-ui.md`. Photos currently serves
> full-size originals; a library of a few thousand photos makes that unusable.

**Goal:** Photos loads quickly on a phone over a home network, without the
server ever trusting a file's extension or letting one hostile image exhaust
memory or CPU.

**Architecture:** A `media` crate that decodes and downscales images under
explicit limits, a derivative cache inside the library root, and a thumbnail
endpoint that generates on demand and caches the result.

## Tasks

1. **`crates/media`** — decode by content (never by extension), enforce pixel
   and memory limits, downscale, and encode JPEG. Pure CPU: no I/O, so it can
   be tested directly and run on a blocking pool.
2. **Derivative storage** — a `.homecloud-derivatives` directory inside the
   library root, skipped by scans, keyed by item id, size, and a fingerprint of
   the source so a changed file cannot serve a stale thumbnail.
3. **`GET /api/v1/items/{id}/thumbnail?size=`** — serves the cached derivative
   or generates it, on the blocking pool, with the same authorization as the
   original.
4. **Caching headers** — thumbnails are private but cacheable; the baseline
   `no-store` must stop overriding a response that sets its own policy.
5. **Photos and Files** — the grid uses thumbnails; the file list shows a small
   preview for images.
6. **Adversarial tests** — a decompression bomb, a file whose extension lies,
   a truncated image, and a non-image are all refused without a panic.

## Self-Review

- Video thumbnails need FFmpeg and a different resource model; out of scope.
- Derivatives are disposable: deleting the cache directory costs a regeneration
  and nothing else, which keeps the "AI/derived data is rebuildable" invariant.
