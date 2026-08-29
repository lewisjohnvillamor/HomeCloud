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
