# Engineering Standards

## 1. Engineering Philosophy

- Evidence over claims.
- TDD for behavior and bug fixes.
- Root cause before fixes.
- Make invalid states hard to represent.
- Prefer explicit, readable code to clever abstraction.
- Measure performance before architecture escalation.
- No completion claim without verification output.

These standards intentionally align with Superpowers, Spartan AI Toolkit, PlanetScale database skills, Vercel Web Interface Guidelines, and Anthropic Code Simplifier.

## 2. Rust

Baseline:
- current stable Rust pinned via `rust-toolchain.toml`;
- `cargo fmt --check`;
- `cargo clippy --all-targets --all-features -- -D warnings` with carefully documented exceptions only;
- `cargo test --workspace`;
- avoid blocking I/O on Tokio async workers;
- cancellation-safe design for long operations where relevant;
- bounded channels/queues;
- structured errors with stable external error codes;
- `unsafe` forbidden by default in application crates unless an ADR/review justifies it.

## 3. TypeScript / React / Next.js

- TypeScript strict mode.
- No `any` without narrowly documented boundary justification.
- Server/client component boundaries are intentional.
- Avoid effect-driven state when derived state suffices.
- Mutations have loading/error/rollback semantics.
- Browser APIs feature-detected.
- Hydration differences tested.
- UI state does not pretend a server mutation succeeded permanently until authoritative confirmation.

## 4. Tests

### Rust
- unit tests for domain invariants;
- integration tests with temporary filesystem roots;
- database integration tests;
- property tests for path/range/parser edge cases where valuable;
- adversarial security tests for path traversal and authorization;
- transfer interruption/resume tests.

### Frontend
- component tests for interaction logic;
- accessibility assertions;
- Playwright E2E for core user journeys;
- visual regression for critical surfaces selectively, not every pixel.

### Required E2E journeys
1. bootstrap owner → add root → index files;
2. upload large file → interrupt → resume → verify;
3. preview/download range-backed media;
4. create/revoke share;
5. mobile photo upload flow;
6. photo timeline → album → slideshow;
7. keyboard-only file management;
8. cross-library access denial;
9. restore from trash/version;
10. restart server during queued background work and recover.

## 5. TDD Cycle

For behavior changes:
1. write failing test;
2. run and prove it fails for the expected reason;
3. implement minimum behavior;
4. run focused test;
5. run related suite;
6. refactor/simplify;
7. run verification again.

Bug fixes begin with a regression test whenever reproducible.

## 6. Systematic Debugging

Before proposing a fix:
1. reproduce and gather evidence;
2. identify working pattern/comparison;
3. state one hypothesis;
4. test minimally;
5. implement only after root cause is supported;
6. verify and record the result.

Three failed fix attempts trigger an architecture/premise review rather than a fourth guess.

## 7. Database

Every meaningful schema/query PR includes:
- workload/cardinality assumptions;
- query shape;
- index reasoning;
- transaction boundary;
- migration risk;
- rollback/roll-forward;
- representative explain plan where performance-sensitive.

Never hold SQL transactions across file transfers, FFmpeg, model inference, or other long external work.

## 8. Performance Budgets

Track:
- p50/p95 endpoint latency;
- time to first file list content;
- memory during 10 GB streamed transfer;
- scanner throughput;
- thumbnail queue age;
- frontend JS bundle per main route;
- LCP/INP/CLS on representative hardware;
- DB slow query threshold.

A “performance optimization” PR needs before/after evidence.

## 9. Accessibility / Interface Gate

All UI PRs apply Vercel Web Interface Guidelines and `EXPERIENCE_SPEC.md`.

Review:
- keyboard;
- focus;
- semantics;
- touch;
- responsive modes;
- error exit path;
- loading behavior;
- reduced motion;
- contrast;
- accessible name/description.

## 10. Code Simplification Gate

After tests pass, review recently changed code using Anthropic Code Simplifier behavior:
- preserve exact functionality;
- reduce unnecessary nesting/indirection;
- improve names;
- prefer explicit control flow;
- consolidate duplicate logic where it clarifies behavior;
- do not create “clever” one-liners;
- rerun tests after simplification.

## 11. Review Order

1. Spec compliance.
2. Security/privacy invariants.
3. Correctness and tests.
4. Database/storage behavior.
5. Accessibility/UX.
6. Performance evidence.
7. Maintainability/simplification.
8. Documentation/migrations/operations.

## 12. Definition of Done

A task is done only when:
- acceptance criteria satisfied;
- tests added and passing;
- formatting/lint/static checks passing;
- security implications reviewed;
- migrations verified where relevant;
- UI states/accessibility verified where relevant;
- docs updated;
- code simplification pass complete;
- no hidden TODO required for the feature to be correct;
- verification evidence is available in the PR/agent summary.
