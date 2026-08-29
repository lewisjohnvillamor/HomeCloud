# Foundation MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` when subagents are available, otherwise `superpowers:executing-plans`, to implement this plan task-by-task.

**Goal:** Create the tested, secure foundation for a Rust + Next.js filesystem-first personal cloud that can boot locally, connect to PostgreSQL, expose health/session scaffolding, and enforce quality gates.

**Architecture:** Rust modular monolith (`crates/*`) behind an Axum API, Next.js web/PWA shell, PostgreSQL metadata database, Docker Compose development profile, native filesystem storage abstraction. No production file mutation, media AI, or sharing in this foundation plan.

**Tech Stack:** Rust stable, Axum, Tokio, SQLx/PostgreSQL, tracing, Next.js, React, TypeScript, Playwright, Docker Compose.

---

## Task 1: Repository skeleton and pinned toolchains

**Deliverable:** A fresh clone has a deterministic Rust workspace and web workspace layout.

**Files:**
- Create `Cargo.toml`
- Create `rust-toolchain.toml`
- Create `crates/domain/Cargo.toml`
- Create `crates/domain/src/lib.rs`
- Create `crates/api/Cargo.toml`
- Create `crates/api/src/lib.rs`
- Create `apps/web/` using current supported Next.js scaffold choices
- Create root package manager/workspace config if needed
- Create `.editorconfig`
- Create `.gitignore`

**Steps:**
1. Add the minimal Rust workspace with `domain` and `api` crates.
2. Write a trivial domain crate unit test that proves the workspace test command is wired.
3. Run `cargo test --workspace` and confirm green.
4. Scaffold Next.js with TypeScript strict mode and no unnecessary sample UI.
5. Add a minimal frontend test command and one smoke test for the root app component/page.
6. Run frontend typecheck/test.
7. Commit: `chore: scaffold rust and web workspaces`.

## Task 2: CI quality baseline

**Deliverable:** CI rejects formatting, clippy, Rust tests, TypeScript errors, frontend lint, and frontend unit test failures.

**Files:**
- Create `.github/workflows/ci.yml`
- Update package scripts/config
- Update `README.md` developer commands if repository README has been imported from this blueprint

**Steps:**
1. Create a deliberate failing lint/test condition locally to prove the command fails, then revert it.
2. Configure Rust CI: fmt, clippy with warnings denied, tests.
3. Configure web CI: lockfile install, lint, typecheck, tests.
4. Add dependency caching without caching generated outputs that can hide failures.
5. Run the same commands locally.
6. Commit: `ci: enforce baseline quality gates`.

## Task 3: Configuration model

**Deliverable:** Typed, validated configuration with safe defaults and no secrets logged.

**Files:**
- Create `crates/api/src/config.rs`
- Create `crates/api/tests/config.rs`
- Create `.env.example`

**Steps:**
1. Write failing tests for missing required DB URL, invalid listen address, and default safe environment behavior.
2. Implement typed config parsing.
3. Add redacted `Debug`/logging behavior for secret-bearing fields.
4. Run focused tests.
5. Run workspace tests.
6. Commit: `feat: add validated server configuration`.

## Task 4: PostgreSQL and migration harness

**Deliverable:** API can connect to PostgreSQL and migrations are deterministic.

**Required domain skill:** PlanetScale Postgres database skill.

**Files:**
- Create `migrations/0001_bootstrap.sql`
- Create `crates/api/src/db.rs`
- Create DB integration test harness
- Create `infra/docker/compose.dev.yml` or root `compose.yml`

**Steps:**
1. Define expected workload for bootstrap tables only; avoid speculative schema.
2. Write an integration test that fails before the migration/database harness exists.
3. Add PostgreSQL service to Compose with healthcheck.
4. Add SQLx pool configuration with bounded connections and timeouts.
5. Create only the minimal migration metadata/table needed for the next task; do not prematurely implement full `DATA_MODEL.md`.
6. Run migration against clean DB.
7. Run integration tests.
8. Recreate DB from scratch and rerun migrations/tests.
9. Commit: `feat: add postgres migration and test harness`.

## Task 5: Axum server, health and readiness

**Deliverable:** Server exposes liveness and DB-aware readiness without leaking internals.

**Files:**
- Create/update `crates/api/src/app.rs`
- Create/update `crates/api/src/main.rs`
- Create `crates/api/tests/health.rs`

**Steps:**
1. Write failing HTTP integration tests for `/health/live` and `/health/ready`.
2. Implement app router through injectable app state.
3. Liveness returns success without DB query.
4. Readiness performs a bounded DB health check.
5. Error response is stable and non-sensitive.
6. Run focused and workspace tests.
7. Commit: `feat: add health and readiness endpoints`.

## Task 6: Structured tracing and request IDs

**Deliverable:** Each HTTP request has a request ID and structured timing; logs do not include secrets.

**Files:**
- Create `crates/observability/` or keep a minimal module in API if extraction would be premature
- Add request middleware tests

**Steps:**
1. Write test for accepted/generated request ID response header behavior.
2. Implement request ID middleware and tracing span fields.
3. Ensure invalid incoming IDs are not blindly trusted if format policy exists.
4. Add panic/error boundary strategy without stack leakage to clients.
5. Run tests.
6. Commit: `feat: add structured request tracing`.

## Task 7: Owner bootstrap domain skeleton

**Deliverable:** Domain types establish a library/user boundary before file APIs exist.

**Files:**
- Update `crates/domain/src/*`
- Add minimal migrations for `users`, `libraries`, `library_members` only if required by implemented flow
- Add repository/service interfaces in appropriate crate
- Add tests

**Steps:**
1. Write domain tests for owner/library invariants.
2. Use PlanetScale Postgres skill to review exact schema/indexes before migration.
3. Implement minimal bootstrap state query/creation transaction.
4. Keep transaction free of network/filesystem work.
5. Run concurrency test proving two bootstrap attempts cannot create two owners for the same deployment policy.
6. Run tests and inspect query plan if a non-trivial lookup/index is introduced.
7. Commit: `feat: establish owner and library boundary`.

## Task 8: Storage abstraction contract

**Deliverable:** Tested storage contract and safe local filesystem root implementation for read-only stat/list primitives only.

**Files:**
- Create `crates/storage/Cargo.toml`
- Create `crates/storage/src/lib.rs`
- Create `crates/storage/src/filesystem.rs`
- Create `crates/storage/tests/filesystem.rs`

**Steps:**
1. Write failing tests for root containment, `..` traversal, absolute path injection, symlink policy, Unicode filename handling, and listing/stat.
2. Define the smallest storage trait needed for stat/list; do not add upload/move/delete yet.
3. Implement canonical root configuration.
4. Implement safe relative path resolution.
5. Keep symlink following disabled by default.
6. Run adversarial tests.
7. Run clippy/workspace tests.
8. Commit: `feat: add safe filesystem storage foundation`.

## Task 9: Web application shell

**Deliverable:** Accessible responsive shell for Home/Files/Photos/Search/More with no fake feature data.

**Required UI guidance:** Vercel Web Interface Guidelines; Taste principles selectively for brand/onboarding, not file-table patterns.

**Files:**
- `apps/web/app/*`
- `apps/web/components/navigation/*`
- tests

**Steps:**
1. Write component/E2E assertions for keyboard navigation and visible focus.
2. Implement desktop sidebar and mobile bottom navigation using semantic links/buttons.
3. Implement responsive behavior without hiding inaccessible duplicate navigation.
4. Add reduced-motion baseline.
5. Verify loading/error/empty shell states.
6. Run keyboard-only Playwright smoke flow at mobile and desktop viewport.
7. Run web lint/typecheck/tests.
8. Commit: `feat: add accessible application shell`.

## Task 10: API client and problem error surface

**Deliverable:** Frontend consumes health/bootstrap endpoints through a typed boundary and renders actionable errors.

**Files:**
- `apps/web/lib/api/*`
- `apps/web/components/*`
- API problem error type in Rust
- tests

**Steps:**
1. Write tests for a successful typed response and a structured problem response.
2. Implement stable API problem JSON with request ID.
3. Implement frontend client mapping without exposing raw backend exceptions.
4. Add one actionable recovery state for API unavailable/readiness failure.
5. Verify slow network/loading state.
6. Run tests.
7. Commit: `feat: connect web shell to typed api errors`.

## Task 11: Security baseline

**Deliverable:** Baseline browser/server security headers, request limits, and explicit CORS/origin behavior.

**Files:**
- API middleware/config
- frontend headers/config where needed
- integration tests

**Steps:**
1. Write failing tests for security header expectations and disallowed cross-origin mutation.
2. Add bounded request-body defaults for metadata routes.
3. Configure CORS as same-origin by default.
4. Add secure content-type behavior and baseline CSP compatible with current app.
5. Document reverse-proxy requirements rather than trusting arbitrary forwarded headers.
6. Run security integration tests.
7. Commit: `security: establish http security baseline`.

## Task 12: Developer one-command environment

**Deliverable:** A contributor can start PostgreSQL + API + web from documented commands.

**Files:**
- `compose.yml` / dev compose
- `Makefile` or `justfile` if chosen
- `README.md`
- scripts only where they remove repeated complexity

**Steps:**
1. Test on a clean checkout/environment or CI job without existing volumes.
2. Start dependencies.
3. Run migrations.
4. Start API and web.
5. Confirm health/readiness and web page.
6. Stop/restart and confirm no destructive reinitialization.
7. Document exact prerequisites and ports.
8. Commit: `docs: add reproducible local development workflow`.

## Task 13: Final review, simplification, verification

**Deliverable:** Foundation branch is reviewable and green.

**Steps:**
1. Run spec-compliance review against `README.md`, `ARCHITECTURE.md`, and this plan.
2. Run security review against `SECURITY_AND_PRIVACY.md` foundation-relevant controls.
3. Apply Anthropic code-simplifier behavior to recently changed code only; preserve behavior.
4. Rerun all tests after simplification.
5. Run:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
# frontend install with lockfile
# frontend lint
# frontend typecheck
# frontend unit/component tests
# foundation Playwright smoke tests
# docker compose config validation
```

6. Start the stack from a clean environment and verify liveness/readiness/UI manually once.
7. Record exact command results in PR summary.
8. Use Superpowers finishing-a-development-branch workflow.

## Self-Review

- This plan intentionally stops before full file scanning/uploads; those deserve independent specs/plans and security review.
- Storage abstraction enters in read-only form first so containment behavior is proven before mutation.
- Database schema is incremental; `DATA_MODEL.md` is a target model, not permission to create every table in Task 4.
- No Redis, Kafka, RustFS, AI model, FFmpeg, or Kubernetes dependency is required for the **foundation milestone**. AI is still a required MVP capability and is intentionally implemented in the final MVP milestone defined in `ROADMAP.md`.
- The first feature plan after this should be **Filesystem Catalog and Reconciliation**, followed by **Transfers and File Operations**.
