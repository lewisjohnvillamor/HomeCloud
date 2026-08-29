# ADR 0004: Password-Backed Sessions Ship Before Passkeys

## Status

Accepted.

## Context

`API_CONTRACT.md` specifies passkeys/WebAuthn as the authentication mechanism,
with optional OIDC. Passkeys need registration and authentication ceremonies,
credential storage, attestation policy, cross-device recovery, and a fallback
for browsers or deployments that cannot use them. None of that can be
half-built safely.

Until some authentication exists, every other capability — catalog, transfers,
sharing — is either unreachable or unprotected. That is the larger risk.

## Decision

The MVP authenticates with a password, verified with Argon2id, and issues an
opaque session token stored hashed in PostgreSQL and delivered as an
`HttpOnly`, `SameSite=Lax` cookie.

Passkeys are added later as an additional credential type against the same
session model, not as a replacement for it. The session layer is deliberately
credential-agnostic so that addition does not require reworking authorization.

## Consequences

- Authorization can be enforced server-side today, on every catalog route.
- A password is a phishable credential; the deployment guidance keeps the
  server on loopback or behind a reverse proxy with TLS.
- Password hashing is CPU-bound and runs on a blocking pool, never on an async
  request executor.
- Adding passkeys requires a `credentials` table and two new endpoint pairs; it
  does not require changing sessions, cookies, or authorization checks.
