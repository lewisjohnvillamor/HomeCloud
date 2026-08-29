# AGENTS.md

This file is the binding operating contract for AI coding agents in Project HomeCloud.

## Read First

Before changing code, read:
1. `README.md`
2. `PRODUCT_SPEC.md`
3. `ARCHITECTURE.md`
4. the domain-specific document relevant to the task
5. `ENGINEERING_STANDARDS.md`
6. `SKILLS_AND_AGENT_WORKFLOW.md`

## Mandatory Workflow

### Feature work
- Use Superpowers brainstorming before implementation when design is not already approved.
- Use Superpowers writing-plans before multi-step implementation.
- Save plans in `docs/superpowers/plans/`.
- Implement in an isolated git worktree/feature branch.
- Use TDD: failing test → minimal implementation → green → refactor.
- Use two-stage review: spec compliance, then code quality.
- Apply code simplification only after tests are green.
- Run verification-before-completion before claiming success.

### Bugs
- Use systematic debugging before proposing a fix.
- Root cause first.
- Add a regression test before/with the fix whenever reproducible.

## Domain Skills

- **Database:** PlanetScale Postgres skill for schema, indexes, transactions, queries, migrations, connection behavior, and performance.
- **Frontend behavior:** Vercel Web Interface Guidelines for every UI change.
- **Visual quality:** Taste Skill principles where appropriate, especially Photos, Memories, shares, onboarding, and public pages. Do not force landing-page aesthetics into dense file-management UI.
- **Quality gates:** Spartan AI Toolkit React/Next.js/security/testing/browser QA practices.
- **Simplification:** Anthropic code-simplifier behavior on recently modified code after tests pass.

## Architecture Invariants

1. Default storage is filesystem-first.
2. PostgreSQL stores metadata/control state, not original file blobs.
3. Paths are not permanent logical identity.
4. Core features work with AI disabled.
5. AI-derived data is disposable/rebuildable.
6. Filesystem watchers are accelerators; reconciliation is authoritative.
7. Authorization is server/domain enforced.
8. Public share capabilities are narrower than user sessions.
9. Heavy blocking work never runs directly on async request executors.
10. Do not add a microservice/Redis/Kafka/Kubernetes dependency without measured need and an ADR.
11. Never automatically delete a file because it appears duplicated.
12. Never claim data is backed up unless a configured verified backup policy supports that claim.

## Security Invariants

- Treat filenames, file contents, archives, media metadata, share input, and paths as untrusted.
- Enforce canonical root containment.
- Do not follow symlinks by default.
- Never log secrets, session tokens, share tokens, full private document contents, OCR contents, or model prompts containing raw user data at normal log levels.
- No cross-library data access.
- No frontend-only authorization.
- No shell interpolation of filenames.
- Resource-limit parsers/transcoders/AI work.

## UI Invariants

- Keyboard-operable primary flows.
- Visible `:focus-visible` state.
- Focus not hidden by sticky/overlay UI.
- Responsive behavior is designed per device class, not merely scaled.
- TV has a distinct remote-friendly interaction model.
- Error states include a recovery action when one exists.
- Reduced-motion support.
- No generic SaaS dashboard cards unless the information genuinely benefits from that form.

## Database Invariants

- Short transactions.
- No transaction remains open while performing filesystem/network/media/model work.
- Indexes derive from real query shapes.
- Migration impact is reviewed.
- Performance-sensitive query changes include explain evidence on representative data when possible.

## Verification Baseline

Before completion, run the applicable subset and report exact results:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace

# web commands finalized once package manager is scaffolded
# lint
# typecheck
# unit/component tests
# playwright core journeys
```

Do not hide failing checks. State the failure and whether it blocks merge.

## Code Style

- Explicit > clever.
- Small cohesive modules.
- Domain names over infrastructure names.
- Avoid speculative generic abstractions.
- Avoid duplicate validation logic; centralize invariants at the correct boundary.
- Comments explain why/constraints, not obvious syntax.
- Public APIs/errors have stable, documented semantics.

## Change Discipline

- Keep commits reviewable and focused.
- Do not mix unrelated refactors into a behavior change.
- Update docs/ADR when architecture or invariants change.
- Update migrations and backup/restore implications together.
- Avoid placeholders in finished production paths.
