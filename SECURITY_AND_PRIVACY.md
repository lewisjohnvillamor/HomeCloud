# Security and Privacy Architecture

## 1. Security Objective

A self-hosted product must assume it may be exposed through a reverse proxy, VPN, tunnel, or public Internet. “It runs at home” is not a security control.

## 2. Threat Model

Protect against:
- stolen/guessed credentials;
- malicious public share recipients;
- path traversal and symlink escapes;
- crafted media/document files;
- CSRF/XSS/session theft;
- brute force and enumeration;
- unauthorized cross-library access;
- SSRF through previews/importers/webhooks;
- malicious filenames and content types;
- decompression/archive bombs;
- resource-exhaustion uploads/transcodes/AI jobs;
- supply-chain compromise;
- compromised client device;
- accidental admin misconfiguration.

Out of initial scope:
- protecting data from a fully compromised server/root administrator;
- sophisticated hardware/firmware attacks.

## 3. Identity

Preferred login:
- passkeys/WebAuthn;
- recovery codes generated once and stored hashed/encrypted as appropriate;
- optional OIDC for advanced deployments.

Password login, if included, uses a modern password hash (Argon2id) and remains optional.

Sessions:
- high-entropy random token;
- only token hash server-side where practical;
- HttpOnly + Secure cookies in HTTPS deployments;
- session rotation on privilege/auth events;
- revoke per-device/all sessions;
- bounded idle and absolute expiry configurable.

## 4. Authorization

Authorization is enforced in Rust domain/application services, not only routes or UI.

Objects belong to a library. Every resource query includes library/user scope or is fetched through an authorization-aware repository/service.

Capabilities/public links have narrower permissions than user sessions.

## 5. Share Links

- capability secret is high entropy;
- DB stores a hash, not plaintext capability;
- optional password uses Argon2id;
- optional expiration and max-use/download count;
- revocation immediate;
- upload-only shares cannot list folder content;
- cache headers appropriate to private content;
- robots/noindex on public share pages by default.

## 6. Filesystem Boundary

- root allowlist configured by administrator;
- canonical path containment check;
- symlink policy explicit;
- all write operations use safe destination resolution;
- temporary files have randomized names and restrictive permissions;
- no shell interpolation of filenames;
- FFmpeg/image tools invoked with argument arrays and sandbox/resource constraints.

## 7. File Processing Sandbox

Media/document parsers receive untrusted input.

Initial baseline:
- process workers with reduced privileges;
- CPU/time/memory limits where platform allows;
- no network access for media processing workers unless required;
- temporary directory quotas;
- input size/pixel/duration limits configurable;
- keep parsing libraries patched.

Stronger sandboxing (containers/seccomp) is a deployment profile, not a reason to omit basic limits.

## 8. Browser Security

Use a restrictive Content Security Policy compatible with the final frontend.

Also:
- no unsafe HTML from filenames/metadata;
- trusted type/sanitization approach for rich previews;
- `X-Content-Type-Options: nosniff` where appropriate;
- clickjacking protection on authenticated UI;
- correct CORS: default same-origin, explicit exceptions only;
- CSRF defense for cookie-auth mutations;
- safe download disposition for untrusted HTML/SVG and active formats.

## 9. Encryption

### In transit
HTTPS expected for non-localhost production use.

### At rest
The app should integrate with disk/filesystem encryption rather than promise magical app-level encryption for every filesystem use case.

Application secrets/config credentials must support encrypted storage using a server-held master key or OS secret mechanism.

A future encrypted-vault mode may encrypt file contents, but it changes preview/search/server processing guarantees and requires a separate threat model.

## 10. Privacy Controls

- local AI default;
- face recognition opt-in;
- location metadata can be hidden from general UI/public shares;
- audit logs avoid content and secrets;
- telemetry opt-in only, with transparent event schema;
- no third-party analytics scripts in the self-hosted UI by default.

## 11. Security Headers / Reverse Proxy

Provide a documented secure reverse proxy profile for Caddy/Traefik/Nginx including:
- TLS;
- forwarded-header trust boundaries;
- upload size behavior;
- long-lived streaming/SSE settings;
- request timeouts;
- HSTS guidance only after operator understands domain consequences.

## 12. Dependency and Supply Chain

CI:
- Rust dependency audit;
- npm lockfile integrity/audit strategy;
- secret scanning;
- SAST where useful;
- container image scan;
- pinned major/minor toolchain policy;
- signed checksums/releases targeted for stable release.

## 13. Security Release Gate

No release candidate if:
- known critical/high exploitable dependency issue without mitigation;
- path containment tests fail;
- cross-library authorization tests fail;
- public share brute-force controls fail;
- session invalidation tests fail;
- upload resource limits can trivially exhaust the service.

## 14. Deployment: Reverse Proxy Requirements

The API binds to loopback by default and terminates no TLS of its own. A
deployment that is reachable from a network must run it behind a reverse
proxy configured as follows.

**The proxy must:**

- terminate TLS and redirect plaintext HTTP to HTTPS;
- pass the browser's `Host` and `Origin` headers through unchanged — the
  server compares them to reject cross-origin state-changing requests;
- **strip client-supplied `X-Forwarded-*` and `Forwarded` headers** before
  setting its own, so a client cannot spoof its address or scheme;
- set `X-Request-Id` only if it generates the value itself; an inbound id
  is accepted only when it is short and alphanumeric, and is replaced
  otherwise;
- apply its own connection, rate, and body limits in front of the API.

**The server does not:**

- trust any forwarded header for authorization decisions;
- emit CORS headers, so no other origin can read an API response;
- serve the web app and the API from different origins in the default
  deployment. Splitting them requires an explicit CORS policy and a
  cookie/CSRF design review first.

Baseline response headers set by the API on every response:
`Content-Security-Policy: default-src 'none'; frame-ancestors 'none'; sandbox`,
`X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`,
`Cross-Origin-Resource-Policy: same-origin`,
`Cross-Origin-Opener-Policy: same-origin`, `Permissions-Policy` with all
optional capabilities disabled, and `Cache-Control: no-store`.

Metadata request bodies are capped at 64 KiB. File transfer endpoints will
define their own, larger, bounded limits when they are implemented.

## 15. Image Derivatives

Thumbnail generation decodes files that arrived from cameras, phones, chat
apps, and downloads, so it is treated as parsing hostile input:

- format is decided by content, never by file extension;
- image dimensions are read from the header and refused above 80 megapixels,
  so a decompression bomb is rejected before any pixel buffer is allocated;
- decoder allocation is capped, and source files above 96 MiB are not read;
- generation runs on a blocking pool, never on an async request executor;
- a damaged or unreadable file produces a normal client error, not a panic and
  not a server error;
- derivatives are written to a cache directory inside the library root, keyed
  by item, size, and a fingerprint of the source, and are excluded from scans.
  Deleting that directory costs a regeneration and nothing else.

## 16. Share Links

A share link is a capability, deliberately narrower than a session:

- it names exactly one item, and for a folder, only what is inside that
  folder — an id from anywhere else resolves to "not found", including a
  sibling folder in the same library;
- it is read-only. Every mutating route requires a session, so a link cannot
  upload, rename, move, delete, or create further links;
- it carries 256 bits of entropy and is stored only as a SHA-256 hash, the
  same rule sessions follow: a database copy yields no working links;
- unknown, expired, and revoked tokens are answered identically, so a visitor
  cannot learn that a link once existed;
- revocation takes effect on the next request, and responses through a link
  are never cached by a shared cache;
- the public page is `noindex`, and the shared view renders paths relative to
  the shared item, so a recipient never learns where it sits in the library.

Not yet implemented: password-protected links, download limits, and per-link
rate limiting. Entropy is currently the whole defence against guessing, which
is sound but is not defence in depth.

## 17. Membership and Invitations

Library membership is the authorization boundary the rest of the system rests
on, so the rules are deliberately few and enforced server-side:

- **Two roles.** A member reads and writes library content. Only the owner
  manages who has access. One function answers "may this caller change
  membership", and every such route goes through it.
- **One owner.** The database rejects a second owner per library, and an
  invitation can only grant `member` — there is no path that promotes anyone.
- **The owner cannot be removed**, in the domain and in the delete statement.
- **Removal is immediate.** Deleting a membership also revokes that person's
  sessions, so an open browser tab loses access at once rather than at expiry.
- **Invitations are bearer tokens** with the same rules as sessions and share
  links: 256 bits of entropy, stored hashed, bounded lifetime (30 days
  maximum), single use, and revocable.
- **An invitation discloses only what it is for** — the library's name and who
  sent it. Unknown, expired, revoked, and already-accepted invitations are
  answered identically.
- **A non-member gets "not found"**, never "forbidden": whether a library
  exists is itself private. A member who lacks a power is told plainly, because
  they already know the library exists.

Not yet implemented: passkeys, account recovery, per-folder permissions, and an
audit log of membership changes (they are logged, but not queryable).

## 18. Passkeys

Passkeys are a second credential against the same session model as a password:

- the relying-party origin comes from configuration, never from a request
  header — a server that trusted `Host` would let an attacker register
  credentials for their own domain;
- challenges are held in memory for five minutes, bound to the account that
  started them, single-use, and capped in number;
- a registration challenge issued for one account cannot be completed by
  another, even with a valid session for that other account;
- the signature counter is written back after every sign-in, which is what
  makes cloned-authenticator detection meaningful;
- a request for a passkey challenge answers identically for an unknown account
  and for one with no passkey, and is subject to the same sign-in throttle;
- stored credentials contain public keys only; nothing here is a secret of the
  user's;
- a passkey never replaces the password, so losing a device does not lock
  someone out of a server sitting in their own house.
