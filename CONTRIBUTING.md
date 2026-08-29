# Contributing

Thanks for helping build Project HomeCloud.

## Before Coding

Read `AGENTS.md` and the relevant product/architecture documents.

For non-trivial features:
1. open/design the problem first;
2. write an approved spec;
3. create an implementation plan under `docs/superpowers/plans/`;
4. work in a focused branch/worktree.

## Pull Requests

A PR should contain:
- problem statement;
- user-visible behavior;
- tests;
- security/data implications;
- screenshots/video for meaningful UI changes;
- verification commands/results;
- migration/rollback notes where applicable;
- performance evidence when performance is a stated goal.

## Commit Scope

Prefer commits that represent independently reviewable behavior. Avoid unrelated formatting/refactors mixed with feature changes.

## Tests

Follow `ENGINEERING_STANDARDS.md`. A bug fix should include a regression test when reproducible.

## Database Changes

Use the PlanetScale Postgres skill/guidance and include query/index/migration reasoning. Do not land a “small” index casually on a table expected to become large.

## UI Changes

Apply `EXPERIENCE_SPEC.md` and Vercel Web Interface Guidelines. Verify keyboard, focus, mobile, loading, error, empty, reduced-motion, and slow-network behavior.

Use Taste principles for visual polish where appropriate, especially media and public-facing surfaces, without sacrificing dense file-management utility.

## Security

Never open public issues containing exploit details for an unpatched vulnerability. Follow `SECURITY.md`.

## Architecture Changes

Changes to a binding invariant require an ADR in `docs/adr/` describing context, decision, consequences, alternatives, and migration implications.

## Licensing

Before first public release, maintainers must select and publish the repository license explicitly. Do not assume code copied from dependencies or skill repositories can be redistributed under this project's license; follow each upstream license.
