# ADR 0005: Object Labels With YOLOX-s Through ONNX Runtime

- Status: Accepted
- Date: 2026-10-10

## Context

Photo understanding is the second step of private AI (`AI_AND_SEARCH.md`):
searching photographs for what is in them. The maintainer asked for a YOLO
detector, to run first on the processor of a typical home server (reference:
Ryzen 5 3600, 48 GB RAM), with a graphics card as a later option.

Three choices had to be made: which detector, which runtime, and how a
deployment without either keeps working.

## Decision

**Detector: YOLOX-s** (Megvii, Apache-2.0 for code and published weights).

- Ultralytics YOLO11 is more accurate (47.0 against 40.5 COCO mAP at the same
  size) but its code *and weights* are AGPL-3.0 or a commercial licence.
  HomeCloud is AGPL today, but `OPEN_SOURCE_STRATEGY.md` keeps Apache-2.0
  open; adopting Ultralytics would close that door, and burden anyone who
  builds on HomeCloud. The accuracy gap is mostly box precision, which
  labelling does not use: the question asked is "is there a dog", not "where
  exactly".
- The InsightFace family is excluded for later work too: its pretrained
  weights are non-commercial research only.

**Runtime: ONNX Runtime, loaded at runtime** through the `ort` crate's
`load-dynamic` mode, minimum runtime 1.20.

- The same contract Tesseract and FFmpeg already sign: the operator installs
  the shared library and points at the model file; with neither, HomeCloud
  builds, starts, and reports the capability absent. Nothing is downloaded at
  build time.
- The same runtime gains NVIDIA support later through its CUDA execution
  provider without changing the model or the code around it. Accepting 1.20+
  lets an operator with an older card (a GTX 960 needs CUDA 12, so ONNX Runtime
  1.26 or earlier) pin a release that still supports it.
- A pure-Rust runtime (`tract`) was considered: no native library, but slower
  on the processor and no graphics card path.

**What is kept: labels only.** For each of the 80 COCO classes, the best
objectness × class score over every candidate; kept at 0.5 or above, at most
twelve per photograph. No boxes, no overlap suppression. Search needs to know
there is a dog; storing where would be keeping more about someone's
photographs than is used.

**Resource limits.** Inference runs on a blocking thread with ONNX Runtime's
intra-op threads capped at half the processor. Labelling is the last step of a
scan, after OCR, and stops after ten minutes per pass; the rest waits for the
next scan. Photographs are decoded under the same limits as thumbnails.

**Model versioning.** Every row records the model's file name and the start of
its SHA-256. A different file is a different stamp and every photograph is
relabelled, never mixed.

## Consequences

- One `unsafe` boundary arrives with the dependency: `ort` loads a native
  library chosen by the operator. HomeCloud's own crates stay
  `#![forbid(unsafe_code)]`, `homecloud-ai` now included.
- The vocabulary is fixed at 80 everyday classes. Open-ended queries ("sunset
  by the lake") need image embeddings, the next step, which can share this
  runtime.
- Labels are derived data: deleted when the owner steps below the `photos`
  profile, rebuilt by a scan, and not part of what a backup must carry.
- CI does not run the model (neither file is fetched); the integration tests
  run it when `HOMECLOUD_OBJECT_MODEL` is set, and skip with a note otherwise.
