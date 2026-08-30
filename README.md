# Project HomeCloud

> **Working title.** A cutting-edge, open-source personal cloud built around one principle: **your files remain yours, on storage you control**.

Project HomeCloud combines the best ideas from Google Drive, Google Photos, local-first computing, modern media libraries, and private AI into one self-hosted experience. It is designed for a home PC, mini-PC, NAS, workstation, or server and is accessible from phones, tablets, laptops, desktops, and TVs.

## Product Promise

- **Your filesystem is the source of truth.** Originals remain ordinary files and folders whenever the filesystem backend is used.
- **No cloud account required.** A fully local deployment works without a third-party SaaS dependency.
- **Fast enough to feel native.** Rust handles indexing, transfer orchestration, media metadata, sync primitives, authorization, and high-throughput APIs.
- **Private intelligence.** Search, OCR, embeddings, face clustering, scene classification, and memory generation can run locally.
- **Every screen matters.** Desktop, mobile, tablet, TV, keyboard, touch, remote control, and casting are first-class experiences.
- **Progressive complexity.** One-user Docker Compose first; multi-user and multi-node scale without requiring a rewrite.

## The Experience

### Drive
Universal file browser, instant previews, resumable uploads, downloads, version history, trash, favorites, tags, comments, share links, full-text search, offline access, WebDAV, and optional S3-compatible access.

### Photos
Timeline, albums, people, places, map, favorites, RAW/Live Photo support, duplicate detection, motion media, automatic mobile backup, editing metadata, and non-destructive organization.

### Memories
Daily/weekly/yearly resurfacing, trips, people, places, events, seasonal collections, “on this day,” cinematic slideshows, music-optional presentation mode, and TV-first playback.

### Ask Your Library
Private natural-language search such as:

- “Show the photos from our Tokyo trip where we were near a train station.”
- “Find the PDF invoice from March that mentions a generator.”
- “Show large videos I have not watched in a year.”
- “What folders are consuming the most redundant storage?”

AI features must degrade gracefully when no local model is configured.

### Devices
Each device is visible as a storage/compute participant. Users can understand where data exists, which items are available offline, backup health, and redundancy without learning storage jargon.

### Living Room
A `/tv` interface with large controls, remote/keyboard navigation, QR pairing, albums, memories, photo frame mode, and slideshow casting.

## Non-Negotiable Principles

1. **No data lock-in.** Metadata can enrich data; it must not become required to recover originals.
2. **Local-first by default.** Internet access enhances sharing and remote access but is not needed for core local use.
3. **Security is a product feature.** Passkeys, least privilege, revocable links, audit trails, encryption choices, and safe defaults.
4. **Fast perceived performance.** Optimistic UI, streaming results, thumbnail pyramids, range requests, virtualized lists, prefetching, and background indexing.
5. **Accessible from day one.** Keyboard, screen reader, focus states, reduced motion, touch target sizing, and TV remote behavior are acceptance criteria.
6. **Open protocols where practical.** WebDAV, S3-compatible optional backend, OpenID Connect, WebAuthn/passkeys, standard media formats, and documented APIs.
7. **Modular monolith before microservices.** Operational simplicity wins until measurement proves a split is needed.

## Reference Stack

| Layer | Default |
|---|---|
| Core API | Rust, Axum, Tokio |
| Data access | SQLx |
| Metadata DB | PostgreSQL |
| Local cache / ephemeral coordination | in-process + optional Redis only when justified |
| Files | Native filesystem backend by default |
| Optional object backend | S3-compatible storage such as RustFS |
| Hashing | BLAKE3 |
| Frontend | Next.js + React + TypeScript |
| UI | Tailwind CSS + accessible headless primitives |
| PWA | Service worker, background/resumable transfer strategy |
| Media | FFmpeg workers + image/video thumbnail pipeline |
| Search | PostgreSQL FTS initially; pluggable vector/semantic index |
| Local AI | provider abstraction for local OCR/embedding/vision models |
| Auth | Passkeys/WebAuthn + secure session cookies; optional OIDC |
| Packaging | Docker Compose first; native binary/desktop packaging later |

## Repository Shape

```text
apps/
  web/                 # Next.js/PWA product and TV UI
crates/
  api/                 # Axum HTTP/WebSocket API
  auth/                # sessions, passkeys, OIDC, permissions
  catalog/             # files, folders, metadata, indexing
  storage/             # filesystem + S3 backend abstraction
  media/               # photos/video metadata and derivatives
  search/              # text, metadata, semantic query orchestration
  sync/                # device state, change feed, conflict primitives
  sharing/             # public/private links and capability grants
  jobs/                # durable job contracts and workers
  domain/              # shared domain types and invariants
  observability/       # tracing, metrics, audit events
services/
  worker-media/        # optional split deployment later
  worker-ai/           # local AI worker; part of MVP capability, runtime-optional
infra/
  docker/
  migrations/
docs/
  adr/
  superpowers/plans/
```

## What Works Today

A single-user deployment is usable end to end:

- **First run** — open the app, create the owner account, and the server
  indexes the library folder it was pointed at.
- **Files** — browse folders, upload (streamed, never overwriting), download
  (with byte ranges, so media seeks), create folders, rename, move, and delete
  to a trash folder that keeps your files on disk. A large file is sent in
  chunks and picked up again where it stopped, so one dropped connection does
  not mean starting a 40 GB video over; a photo still goes in one request,
  because three round trips to move two megabytes helps nobody.
- **Photos** — images and videos, grouped by the month the picture was *taken*
  — read from the camera's own metadata, because copying a folder to a new disk
  rewrites every file date — served as generated thumbnails (a poster frame for
  a video) so a large library loads on a phone. Thumbnails respect the way the
  camera was held, so a portrait photo is not a sideways tile.
  Both preview inline in the file list too. Video previews need FFmpeg on the
  server; without it everything else still works and videos simply have no
  preview.
- **Albums and favorites** — star a picture and it is yours: in a shared
  library, what someone stars is theirs alone. Albums are the opposite — a set
  the whole library shares, arranged in the order you put them in. Neither owns
  any bytes, so deleting an album keeps every photo in it, and renaming or
  moving a file leaves both intact.
- **Search** — finds a file by its name *or* by the text inside it (plain text
  and PDF today), with a snippet showing the matching passage.
- **People** — invite someone to the library with a one-time link; they create
  their own account and can then read and add files. Only the owner manages
  membership, and removing someone ends their sessions immediately.
- **Sharing** — create a read-only public link to one file or folder, with an
  optional expiry and an optional password, revocable at any moment. A link
  grants nothing else: no browsing, no uploads, no other item. A protected
  link discloses nothing at all — not even the file's name — until the
  password is given.
- **Living room** — `/tv` is a remote-first photo wall and slideshow: arrows to
  move, Enter to play or pause, Escape to go back. It shows deterministic
  collections (on this day, recently added) that need no AI. A television with
  no keyboard shows a short code and a QR square instead of a password form;
  someone signed in approves it from their phone, and the screen gets a
  credential that can show photos from one library and nothing else. Paired
  screens are listed in More and can be disconnected there.
- **More** — rescan the library, manage people, disconnect televisions, audit
  and revoke shared links, restore from trash, replace your recovery code,
  sign out.
- **Passkeys** — register a device as a passkey and sign in with it instead of
  a password. Needs `HOMECLOUD_PUBLIC_ORIGIN` set, because WebAuthn binds a
  credential to a domain; without it the option is hidden rather than broken.
- **Recovery** — a recovery code is generated at setup and shown once. There is
  no email reset on a server in someone's house, so the code is the way back in
  after a forgotten password: it sets a new password, ends every existing
  session, and is replaced with a fresh one in the same step.
- **Security** — Argon2id passwords, `HttpOnly` session cookies, throttled
  sign-in, server-enforced library membership on every route, canonical-root
  containment, and no symlink following.

Not yet built: upload request links, file versions, video transcoding for
playback, offline sync, and the local AI features. See `ROADMAP.md`.

## Local Development

Prerequisites: Rust (version pinned by `rust-toolchain.toml`), Node.js 22+,
pnpm 10+, and Docker (for PostgreSQL only — the API and web app run
natively). FFmpeg is optional and only used for video poster frames.

```bash
make setup     # install web dependencies, create .env from .env.example
make db-up     # start PostgreSQL and wait for it to accept connections
make dev       # run the API and the web app together
```

Ports: web app on <http://127.0.0.1:3000>, API on <http://127.0.0.1:8080>,
PostgreSQL on `127.0.0.1:5432` (loopback only). The web dev server proxies
`/api` and `/health` to the API, so the browser always talks to one origin.

`HOMECLOUD_API_ORIGIN` is read when the web app builds, not when it starts:
`next build` compiles the proxy rules into the build output. A deployment
behind a real reverse proxy does not need it — both apps are served from one
origin — but a build made for one API port cannot be pointed at another
without rebuilding.

Verify the stack:

```bash
curl http://127.0.0.1:8080/health/live      # {"status":"ok"}
curl http://127.0.0.1:8080/health/ready     # {"status":"ready"} once the DB is up
curl http://127.0.0.1:3000/api/v1/bootstrap # {"needs_owner":true} on a fresh install
```

The API applies pending migrations at startup and skips already-applied
ones, so restarting never reinitialises existing data. `make db-reset`
deletes the development database and its volume; nothing else does.

If you prefer to run PostgreSQL yourself, point `HOMECLOUD_DATABASE_URL`
at it and skip `make db-up`. All configuration is documented in
`.env.example`.

## Developer Commands

```bash
make check       # everything CI runs except the browser tests
make check-rust  # cargo fmt --check, clippy -D warnings, cargo test
make check-web   # eslint, tsc --noEmit, vitest
make e2e         # Playwright UI checks, desktop and mobile viewports
make e2e-full    # full-stack journeys: real API, real PostgreSQL, built web app
```

`make e2e-full` recreates the `homecloud_e2e` database before it runs, because
the journeys start at first-run setup. Point it at a different server with
`HOMECLOUD_E2E_ADMIN_URL` and `HOMECLOUD_E2E_DATABASE_URL`.

`cargo test --workspace` runs database integration tests when `DATABASE_URL`
points at a PostgreSQL server it may create and drop databases on; without
it those tests skip and say so. CI always sets it.

CI (`.github/workflows/ci.yml`) runs exactly these commands; a check that
fails locally fails in CI.

## Documentation Map

- `PRODUCT_SPEC.md` — scope, personas, requirements, acceptance criteria.
- `EXPERIENCE_SPEC.md` — cutting-edge UX and interaction system.
- `ARCHITECTURE.md` — system architecture and component boundaries.
- `STORAGE_AND_SYNC.md` — filesystem model, indexing, hashes, devices, conflicts.
- `DATA_MODEL.md` — initial PostgreSQL schema and invariants.
- `API_CONTRACT.md` — API style, core resources, transfers, realtime events.
- `AI_AND_SEARCH.md` — private AI and semantic search design.
- `SECURITY_AND_PRIVACY.md` — threat model and controls.
- `ENGINEERING_STANDARDS.md` — coding, test, performance, observability standards.
- `SKILLS_AND_AGENT_WORKFLOW.md` — required AI-agent methodology.
- `ROADMAP.md` — phased delivery and release gates.
- `CONTRIBUTING.md` — contributor workflow.
- `SECURITY.md` — vulnerability reporting and security policy.
- `docs/adr/` — binding architecture decisions.
- `docs/superpowers/plans/2026-08-29-foundation-mvp.md` — first implementation plan.
- `docs/superpowers/plans/2026-08-29-mvp-final-ai.md` — final MVP private AI implementation plan.

## Definition of v1

v1 is not “every Google feature.” It is a polished personal cloud with:

- filesystem-backed library and external folder import;
- files/folders, previews, upload/download, trash, versioning, share links;
- photo timeline, albums, favorites, EXIF, map metadata, memories, slideshow;
- mobile-friendly PWA with automatic/manual backup workflow;
- TV/presentation mode;
- filename/metadata/full-text search;
- local OCR, text/image embeddings, semantic search, and Ask Your Library;
- passkeys and secure session management;
- background indexer, media derivative workers, and bounded local AI jobs;
- backup/export/restore documentation;
- observability and upgrade-safe migrations.

AI is deliberately the **final MVP milestone**: phases 0–5 establish trustworthy storage, files, sharing, photos, memories, and TV; phase 6 adds private intelligence without making AI a dependency for data access. Post-MVP device/offline federation work begins after that gate.

See `ROADMAP.md` for sequencing.
