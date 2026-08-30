# Catalog, Transfers, Auth, and Product UI Implementation Plan

> Follows `2026-08-29-foundation-mvp.md`. That plan stopped deliberately before
> file mutation; this one takes the foundation to a usable personal cloud.

**Goal:** A person can open HomeCloud in a browser, create the owner account,
point it at a folder, and then browse, upload, download, organise, and search
their own files and photos — with authorization enforced on the server and the
filesystem remaining the source of truth.

**Architecture:** New `crates/auth` (sessions, password verification) and
`crates/catalog` (items, scanning, reconciliation) alongside the existing
`domain`, `storage`, and `api` crates. PostgreSQL holds metadata only;
originals stay ordinary files inside the library root.

---

## Phase A — Identity and access

1. Password-backed owner account (Argon2id), hashed in a blocking pool.
2. Opaque session tokens, stored hashed, delivered as `HttpOnly` cookies.
3. `GET /api/v1/session`, `POST /api/v1/setup`, `POST /api/v1/auth/login`,
   `POST /api/v1/auth/logout`.
4. Server-enforced library membership on every catalog route. No frontend-only
   authorization, no cross-library reads.

## Phase B — Catalog and reconciliation

1. `items` table keyed by stable id, with library-relative path as data rather
   than identity.
2. Reconciliation walk of the library root: insert new, update changed, mark
   vanished items as missing. Never deletes a file.
3. Scan runs as a background task with observable status, never on the request
   executor.
4. `GET /api/v1/libraries`, `GET /api/v1/items/{id}/children`,
   `POST /api/v1/libraries/{id}/scan`.

## Phase C — Transfers and file operations

1. Streaming download with HTTP Range support.
2. Streaming upload, written to a temporary file inside the root and renamed
   into place, with its own bounded size limit separate from metadata routes.
3. Create folder, rename, move, and delete-to-trash. Trash moves the file into
   an application-managed area; nothing is unlinked implicitly.
4. Catalog updates and filesystem changes stay consistent, with the filesystem
   authoritative on conflict.

## Phase D — Product UI

1. Setup and sign-in flows.
2. Files: breadcrumb navigation, folder listing, upload, download, new folder,
   rename, delete, restore.
3. Photos: image items from the catalog, in a responsive grid.
4. Search: name search across the library.
5. Keyboard operability and honest loading/empty/error states throughout.

## Phase E — Verification

Full gate run, adversarial authorization tests, and a manual pass over the
running stack at desktop and mobile viewports.

## Self-Review

- Passkeys remain the target for authentication; a password-backed session is
  the smallest credible thing that makes the product usable and is recorded in
  an ADR rather than implied.
- Thumbnails, transcoding, media derivatives, and AI stay out of scope: they
  need their own resource-limit and security review.
- Sharing stays out of scope; share capabilities must be narrower than a
  session and deserve a dedicated plan.
