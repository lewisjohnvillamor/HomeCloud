# Roadmap

> **Status (August 2026).** Phase 0 is complete. Phases 1 and 2 are complete
> for the filesystem backend: the catalog reconciles against disk, and
> transfers, folders, renames, moves, and trash/restore all work. Phase 3 has
> landed apart from account recovery and upload request links:
> password-backed sessions, server-enforced library membership, invitations
> with per-role powers, and revocable public share links. Phase 4 has the Photos
> timeline over indexed images with generated thumbnails; video derivatives and
> transcoding are still ahead. Phase 6 has its
> non-AI half: document text is extracted during a scan and searched alongside
> file names; OCR, embeddings, and Ask Your Library are still ahead. Phase 5
> has the `/tv` interface, a remote-driven slideshow, and deterministic
> memories; QR pairing and a TV-scoped token are not built.

The project is intentionally phased. Each phase must feel useful before the next one begins.

**MVP boundary:** Phases **0 through 6** are the MVP. Phase 6, **Search and Private AI**, is intentionally the final MVP milestone so AI lands on top of a trustworthy storage/media foundation instead of blocking it.

## Phase 0 — Foundation and Developer Experience

**Goal:** reproducible skeleton with quality gates.

Deliver:
- Rust workspace + Axum API;
- Next.js TypeScript app;
- PostgreSQL + migrations;
- Docker Compose development profile;
- structured tracing;
- health/readiness;
- CI: fmt, clippy, tests, frontend lint/typecheck/test;
- AGENTS/skills integration;
- architecture and security regression harness scaffolding.

Exit gate:
- fresh clone boots locally from documented command;
- all checks green;
- no application feature yet depends on mock-only architecture.

## Phase 1 — Filesystem Catalog

Deliver:
- storage roots;
- safe scanner;
- incremental reconciliation;
- file/folder catalog;
- folder list/grid UI;
- metadata preview;
- BLAKE3 background hashing;
- exact duplicate reporting only.

Exit gate:
- index 1M synthetic entries without UI loading all rows;
- restart/resume scan;
- path traversal/symlink tests pass.

## Phase 2 — File Operations and Transfers

Deliver:
- upload session;
- resumable upload;
- range download;
- create/rename/move/copy;
- trash/restore;
- versions;
- transfer tray;
- conflict/name collision UX.

Exit gate:
- interrupted 10+ GB synthetic upload resumes without corruption;
- cross-filesystem move fallback verified;
- version restore verified.

## Phase 3 — Auth, Multi-user, Sharing

Deliver:
- owner bootstrap;
- passkeys;
- recovery;
- libraries/members;
- share links;
- upload request links;
- rate limits/audit events;
- public file/folder/album pages.

Exit gate:
- cross-library authorization security suite;
- revoked share immediately fails;
- brute-force protections verified.

## Phase 4 — Photos and Media

Deliver:
- EXIF metadata;
- image thumbnails;
- video poster/proxy;
- timeline;
- favorites;
- albums;
- map metadata/view;
- phone-friendly backup flow;
- RAW/motion-photo capability matrix.

Exit gate:
- large photo library scroll performance target met;
- background processing bounded;
- originals always downloadable.

## Phase 5 — Memories and TV

Deliver:
- deterministic memories engine;
- On This Day;
- trips/date/location clusters;
- memory hide controls;
- slideshow;
- `/tv` UI;
- QR pairing;
- photo-frame mode.

Exit gate:
- TV fully keyboard/remote navigable;
- memories function with AI disabled.

## Phase 6 — Search and Private AI — FINAL MVP MILESTONE

Deliver:
- full-text extraction/search;
- local OCR provider for scans/screenshots where extraction is insufficient;
- local text and image embedding providers;
- **Ask Your Library** natural-language search over files, documents, and photos;
- semantic query combined with deterministic type/date/path/size/location filters;
- image caption/tag inference for discoverability;
- AI-assisted memory titles/ranking while deterministic memories remain authoritative;
- optional face clustering behind explicit library-owner opt-in;
- model/provider status UI, queue visibility, pause/resume, and resource controls;
- AI metadata model-versioning, deletion, and rebuild controls;
- local-first provider abstraction with remote providers disabled unless explicitly configured.

Exit gate:
- a fresh supported installation can enable the documented local AI profile without a proprietary cloud account;
- natural-language queries can retrieve both document and photo examples from a representative test library;
- OCR results are searchable and inherit source authorization;
- turning AI off leaves Files, Photos, Memories, sharing, and deterministic search healthy;
- derived metadata deletion/rebuild and model upgrade paths are tested;
- AI jobs are bounded and cannot starve uploads/previews;
- no raw user data is sent remotely by default;
- AI-generated/inferred metadata is visibly distinguishable from authoritative catalog metadata.

### MVP Completion Gate

MVP is complete only when Phases 0–6 all meet their exit gates. AI functionality is therefore **in MVP**, not a post-MVP experiment, while remaining optional at runtime for privacy, hardware, and reliability reasons.

## Phase 7 — Offline and Devices — POST-MVP

Deliver:
- PWA offline pin sets;
- server change feed;
- device cursors;
- conflict preservation;
- device availability UI;
- optional WOL hook.

Exit gate:
- offline edit/conflict never silently destroys either version.

## Phase 8 — Ecosystem — POST-MVP

Candidates:
- WebDAV;
- optional S3/RustFS backend;
- OIDC;
- Collabora/OnlyOffice integration;
- native Tauri desktop wrapper;
- mobile-native helper for background camera upload if PWA platform restrictions require it;
- plugin/extension SDK;
- federated server-to-server sharing;
- verified multi-node replicas.

Each candidate requires its own spec and Superpowers plan.

## Release Channels

### `0.x alpha`
Data model may evolve; backups/export must still be documented.

### `0.x beta`
Migration compatibility commitments begin; security review broadened.

### `1.0`
Requires:
- upgrade path from latest supported beta;
- stable backup/restore contract;
- documented threat model;
- independent security review if project resources allow;
- performance baseline;
- accessibility audit of core flows;
- no known critical/high exploitable issue without mitigation.
