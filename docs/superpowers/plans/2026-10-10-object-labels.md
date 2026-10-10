# Object Labels (YOLOX-s) Implementation Plan

**Goal:** a library owner who turns on photo understanding can search their
photographs for what is in them — "dog", "bicycle", "cake" — with the model
running on the processor they already have and nothing leaving the server.

**Decisions (agreed with the maintainer, 2026-10-10):** YOLOX-s rather than
Ultralytics YOLO, run through ONNX Runtime, processor first, graphics card
later and optional. Recorded in `docs/adr/0005-object-labels-with-yolox.md`.

**Reference hardware:** Ryzen 5 3600, 48 GB RAM, GTX 960. The GTX 960 is
not used by this slice.

## Shape

- The `photos` profile (already stored in `ai_settings`) now means
  something: OCR plus object labels. `people` includes it too.
- ONNX Runtime is loaded at runtime from a shared library, exactly as
  Tesseract and FFmpeg are found at runtime. No library, or no model file,
  means the capability is absent — reported, never an error.
- Only *which* things appear in a photo is stored, not where. The decoder
  takes the most confident score per class over every anchor, so no box
  decoding and no NMS: labels are what search needs, and nothing more is
  kept than is used.
- Labels live in their own table, `item_objects`, one row per image, stamped
  with the model that produced them. A different model file is a different
  stamp, and every image is relabelled — rebuild without overwrite-in-place.
- Search reads that table in the same ranked query it already runs, and
  reports such a hit as `matched: "objects"` with the labels, so the
  interface can say "recognised in the photo" rather than present an
  inference as a fact.
- Turning photo understanding off (to `text` or `off`) deletes every label.

## Tasks

1. **Labels vocabulary** — the 80 COCO classes, each with the words a person
   would search for it by ("people" for person, "sofa" for couch). Tests:
   class count, every class searchable by its own name, plural forms.
2. **Decoding** — per-class maximum of objectness × class score over the
   model output, thresholded. Tests on synthetic tensors: below threshold is
   nothing, duplicates collapse to one label with the best score, malformed
   output is an error not a panic.
3. **Letterboxing** — resize to fit 640×640 preserving aspect ratio, pad
   with 114, BGR planar, unnormalised, top-left aligned as YOLOX trains.
   Tests on small synthetic images: shape, padding value, channel order.
4. **Detector** — load the runtime and model, run on a blocking thread with
   intra-op threads capped at half the processor so uploads and previews
   stay responsive. Integration test against the real model, skipped with a
   note when `HOMECLOUD_ONNXRUNTIME`/`HOMECLOUD_OBJECT_MODEL` are not set.
5. **Profile and capabilities** — `includes_objects()`, `Capabilities.objects`,
   `supported()` reaches `photos` only when both OCR and labels are present.
6. **Migration `0023_object_labels.sql`** — table, GIN index on the search
   vector, `(library_id, model)` index for "what needs relabelling".
7. **Scan step** — last in the pipeline, after OCR, bounded per pass, decoded
   under the same limits as thumbnails.
8. **Settings API** — `objects_available`, `objects_pending`; dropping below
   `photos` deletes labels. Tests.
9. **Search** — join, rank, `matched: "objects"`, `objects: [...]`. Test with
   labels inserted directly (no model needed).
10. **Web** — offer "Recognise what is in photos" when the server can; show
    the recognised labels on a search hit as inferred, not as fact.
11. **Docs** — ADR, `AI_AND_SEARCH.md` status, `.env.example`, README setup.

## Verification

`cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features
-- -D warnings`, `cargo test --workspace` with PostgreSQL and with the real
model and runtime, and the web lint/typecheck/unit tests.
