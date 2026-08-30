# API Contract

## 1. Style

- Versioned HTTP API under `/api/v1`.
- JSON for metadata/control operations.
- Streaming bodies for file transfer.
- SSE for server-to-client progress/change events initially.
- Problem Details style errors (`application/problem+json`) where practical.
- Idempotency keys for retriable create/finalize operations.

## 2. Authentication

Browser sessions use secure, HttpOnly, SameSite cookies with CSRF defense appropriate to the final route design. Public share capability secrets use dedicated endpoints/tokens and never become general user sessions.

## 3. Core Resources

> **Implemented today.** The endpoints below marked *(built)* exist and are
> covered by tests; the rest are the target contract. Authentication ships as
> a password credential rather than passkeys — see
> `docs/adr/0004-password-sessions-before-passkeys.md`.
>
> ```text
> GET    /health/live                              (built) liveness, no database
> GET    /health/ready                             (built) readiness, bounded DB probe
> GET    /api/v1/bootstrap                         (built) does this deployment need an owner
> POST   /api/v1/setup                             (built) create the owner, library, and session
> POST   /api/v1/auth/login                        (built) sign in
> POST   /api/v1/auth/logout                       (built) sign out
> GET    /api/v1/session                           (built) who is signed in
> GET    /api/v1/libraries                         (built) libraries this account can see
> GET    /api/v1/libraries/{id}/browse?path=       (built) folder listing with breadcrumb
> GET    /api/v1/libraries/{id}/photos             (built) images, newest first
> GET    /api/v1/libraries/{id}/search?q=          (built) name search
> GET    /api/v1/libraries/{id}/trash              (built) trashed items
> POST   /api/v1/libraries/{id}/scan               (built) start a background reconciliation
> GET    /api/v1/libraries/{id}/scan               (built) scan status
> POST   /api/v1/libraries/{id}/folders            (built) create a folder
> POST   /api/v1/libraries/{id}/upload?path=       (built) streaming upload, never overwrites
> GET    /api/v1/items/{id}                        (built) item metadata
> GET    /api/v1/items/{id}/children               (built) folder contents by id
> GET    /api/v1/items/{id}/content                (built) download, supports one byte range
> GET    /api/v1/items/{id}/thumbnail?size=       (built) generated preview: small|medium|large
> POST   /api/v1/items/{id}/move                   (built) rename or move
> DELETE /api/v1/items/{id}                        (built) move to trash
> POST   /api/v1/items/{id}/restore                (built) restore from trash
> POST   /api/v1/items/{id}/shares                 (built) create a public link
> GET    /api/v1/items/{id}/shares                 (built) links for one item
> GET    /api/v1/libraries/{id}/shares             (built) every live link, for auditing
> DELETE /api/v1/shares/{id}                       (built) revoke a link
> GET    /api/v1/public/{token}                    (built) what a link points at — no session
> GET    /api/v1/public/{token}/content            (built) download through a link
> GET    /api/v1/public/{token}/thumbnail          (built) preview through a link
> ```
>
> Errors use the problem shape described in section 6, served as
> `application/problem+json` with the request id that appears in the logs.

### Auth
- `POST /api/v1/auth/passkeys/register/options`
- `POST /api/v1/auth/passkeys/register/verify`
- `POST /api/v1/auth/passkeys/login/options`
- `POST /api/v1/auth/passkeys/login/verify`
- `POST /api/v1/auth/logout`
- `GET /api/v1/session`

### Libraries and roots
- `GET /api/v1/libraries`
- `GET /api/v1/roots`
- owner/admin-only root management endpoints

### Items
- `GET /api/v1/items/{id}`
- `GET /api/v1/items/{id}/children?cursor=&limit=&sort=`
- `PATCH /api/v1/items/{id}`
- `POST /api/v1/items/{id}/move`
- `POST /api/v1/items/{id}/copy`
- `DELETE /api/v1/items/{id}` → trash by default
- `POST /api/v1/items/{id}/restore`
- `GET /api/v1/items/{id}/versions`

### Preview/download
- `GET /api/v1/items/{id}/preview`
- `GET /api/v1/items/{id}/content` with Range support
- `GET /api/v1/items/{id}/thumbnail?size=`

### Upload sessions
- `POST /api/v1/uploads`
- `HEAD /api/v1/uploads/{id}`
- `PATCH /api/v1/uploads/{id}` or protocol-specific chunk operation
- `POST /api/v1/uploads/{id}/complete`
- `DELETE /api/v1/uploads/{id}`

Upload creation returns:
- session ID;
- accepted offset/chunk policy;
- destination logical ID/path;
- expiry;
- limits.

### Search
- `GET /api/v1/search?q=&type=&after=&before=&root=&tag=`
- `POST /api/v1/search/semantic` only when enabled; query body allows structured filters.

Lexical search remains the primary endpoint; semantic search must not be required for basic operation.

### Albums/memories
- album CRUD;
- add/remove/reorder album items;
- memory feed;
- dismiss/hide/save memory.

### Shares
- `POST /api/v1/shares`
- `GET /api/v1/shares`
- `PATCH /api/v1/shares/{id}`
- `DELETE /api/v1/shares/{id}` revoke
- public routes under `/s/{capability}` with strict separation from authenticated API.

### Devices/change feed
- `GET /api/v1/devices`
- `POST /api/v1/devices/{id}/revoke`
- `GET /api/v1/changes?after=&limit=`

### Events
- `GET /api/v1/events` SSE

## 4. Pagination

Cursor-based pagination only for large mutable collections. Cursors are opaque. API never asks clients to calculate raw DB offsets for large file timelines.

## 5. Concurrency

Resources expose an `etag`/version value. Mutating operations that can overwrite user changes support `If-Match` or equivalent version precondition.

## 6. Errors

Example shape:

```json
{
  "type": "https://project.example/problems/name-conflict",
  "title": "A file with that name already exists",
  "status": 409,
  "code": "ITEM_NAME_CONFLICT",
  "detail": "Choose a different name or select Replace.",
  "request_id": "..."
}
```

Never return internal filesystem paths, stack traces, SQL, secrets, or host topology to untrusted clients.

## 7. Rate Limits

Separate buckets for:
- auth attempts;
- public share requests;
- share password attempts;
- metadata API;
- transfer sessions;
- expensive semantic search.

Self-hosted operators can tune limits, but safe defaults remain enabled.

## 8. API Compatibility

- Additive compatible changes within v1 where possible.
- Breaking changes require `/v2` or a documented compatibility window.
- Event payloads include event version.
- Jobs include job kind version for resumable upgrades.
