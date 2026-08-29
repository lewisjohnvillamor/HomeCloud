# Quality Gates

## Gate A — Spec
- user problem and non-goals clear;
- data-loss behavior defined;
- security/privacy implications identified;
- acceptance criteria testable.

## Gate B — Plan
- concrete files/tasks;
- test-first steps;
- migration and rollback where relevant;
- no placeholder implementation steps;
- each task independently reviewable.

## Gate C — Implementation
- TDD evidence;
- focused tests green;
- no architecture invariant violation;
- domain skill review performed.

## Gate D — Review
1. spec compliance;
2. security/data safety;
3. correctness/tests;
4. database/storage;
5. accessibility/UX;
6. performance;
7. maintainability.

## Gate E — Simplification
Recently modified code simplified without behavioral change, followed by rerun of tests.

## Gate F — Verification
Applicable full checks pass. Completion summary includes exact commands/results.

## Gate G — Release
- migrations tested from supported prior version;
- backup/restore smoke test;
- dependency/security scan;
- core Playwright journeys;
- release notes with breaking/operational changes;
- container image provenance/checksum strategy when release tooling is mature.
